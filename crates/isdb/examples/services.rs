use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use isdb::{AribString, Pid};

#[derive(Debug)]
struct AppArgs {
    path: PathBuf,
    show_events: bool,
}

impl AppArgs {
    const HELP: &str = "\
各サービスの情報を表示するコマンド

USAGE:
  services [OPTIONS] [PATH]

FLAGS:
  -h, --help    このヘルプを表示する
  --show-events 番組名を表示する

ARGS:
  <PATH>        サービス情報を表示するTSファイルのパス
";

    pub fn parse() -> Result<AppArgs, Box<dyn std::error::Error>> {
        let mut args = pico_args::Arguments::from_env();

        if args.contains(["-h", "--help"]) {
            println!("{}", Self::HELP);
            std::process::exit(0);
        }

        let show_events = args.contains("--show-events");

        Ok(AppArgs {
            path: args.free_from_str()?,
            show_events,
        })
    }
}

#[derive(Debug)]
struct Event {
    event_id: u16,
    name: AribString,
}

#[derive(Debug)]
struct Service {
    service_id: u16,
    provider_name: AribString,
    service_name: AribString,
    events: Vec<Event>,
}

impl Service {
    fn has_event(&self, event_id: u16) -> bool {
        self.events.iter().any(|ev| ev.event_id == event_id)
    }
}

struct Filter {
    services: Vec<Service>,
}

impl Filter {
    pub const fn new() -> Filter {
        Filter {
            services: Vec::new(),
        }
    }

    fn find_service(&mut self, service_id: u16) -> Option<&mut Service> {
        self.services
            .iter_mut()
            .find(|svc| svc.service_id == service_id)
    }
}

impl isdb::demux::Filter for Filter {
    fn on_pes_packet(&mut self, _: &isdb::Packet, _: &isdb::pes::PesPacket) {}

    fn on_packet(&mut self, packet: &isdb::Packet) -> Option<isdb::demux::PacketType> {
        match packet.pid() {
            Pid::PAT | Pid::SDT | Pid::H_EIT | Pid::M_EIT | Pid::L_EIT => {
                Some(isdb::demux::PacketType::Psi)
            }
            _ => None,
        }
    }

    fn on_psi_section(&mut self, packet: &isdb::Packet, psi: &isdb::psi::PsiSection) {
        match packet.pid() {
            Pid::PAT => {
                let Some(pat) = isdb::table::Pat::read(psi) else {
                    return;
                };

                for program in &*pat.pmts {
                    if self.find_service(program.program_number.get()).is_some() {
                        continue;
                    }

                    self.services.push(Service {
                        service_id: program.program_number.get(),
                        provider_name: AribString::new(),
                        service_name: AribString::new(),
                        events: Vec::new(),
                    });
                }
            }

            Pid::SDT => {
                let sdt = match isdb::table::Sdt::read(psi) {
                    Some(isdb::table::Sdt::Actual(sdt)) => sdt,
                    _ => return,
                };

                for svc in &*sdt.services {
                    let Some(service) = self.find_service(svc.service_id) else {
                        continue;
                    };
                    let Some(sd) = svc.descriptors.get::<isdb::desc::ServiceDescriptor>() else {
                        continue;
                    };

                    if service.provider_name.is_empty() {
                        service.provider_name = sd.service_provider_name.to_owned();
                    }
                    if service.service_name.is_empty() {
                        service.service_name = sd.service_name.to_owned();
                    }
                }
            }

            Pid::H_EIT | Pid::M_EIT | Pid::L_EIT => {
                let eit = match isdb::table::Eit::read(psi) {
                    Some(isdb::table::Eit::ActualPf(eit)) => eit,
                    _ => return,
                };
                let Some(service) = self.find_service(eit.service_id) else {
                    return;
                };
                let Some(event) = eit.events.first() else {
                    return;
                };
                if service.has_event(event.event_id) {
                    return;
                }
                let Some(sed) = event.descriptors.get::<isdb::desc::ShortEventDescriptor>() else {
                    return;
                };

                service.events.push(Event {
                    event_id: event.event_id,
                    name: sed.event_name.to_owned(),
                });
            }

            _ => {}
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = AppArgs::parse()?;

    env_logger::init();

    let f = File::open(&*args.path)?;
    let f = BufReader::with_capacity(188 * 1024, f);

    let mut demuxer = isdb::demux::Demuxer::new(Filter::new());
    for packet in isdb::Packet::iter(f) {
        demuxer.handle(&packet?);
    }

    let services = demuxer.into_filter().services;
    for svc in services {
        // サービスIDとサービス名
        print!(
            "{}(0x{:04X}) {}",
            svc.service_id,
            svc.service_id,
            svc.service_name.display(Default::default()),
        );
        // 事業者名
        if !svc.provider_name.is_empty() {
            print!(" - {}", svc.provider_name.display(Default::default()));
        }

        // 番組名
        if args.show_events && !svc.events.is_empty() {
            print!(" (");
            for (i, event) in svc.events.into_iter().enumerate() {
                if i != 0 {
                    print!(" | ");
                }
                print!("{}", event.name.display(Default::default()));
            }
            print!(")");
        }

        println!();
    }

    Ok(())
}
