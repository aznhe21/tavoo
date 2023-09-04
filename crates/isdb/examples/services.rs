use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use isdb::psi::table::{EventId, ServiceId};
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
    event_id: EventId,
    name: AribString,
}

#[derive(Debug)]
struct Service {
    service_id: ServiceId,
    provider_name: AribString,
    service_name: AribString,
    events: Vec<Event>,
}

impl Service {
    fn has_event(&self, event_id: EventId) -> bool {
        self.events.iter().any(|ev| ev.event_id == event_id)
    }
}

#[derive(Debug, Clone, Copy)]
enum Tag {
    Pat,
    Sdt,
    Eit,
}

struct Filter {
    repo: isdb::psi::Repository,
    services: Vec<Service>,
}

impl Filter {
    pub fn new() -> Filter {
        Filter {
            repo: isdb::psi::Repository::new(),
            services: Vec::new(),
        }
    }

    fn find_service(&mut self, service_id: ServiceId) -> Option<&mut Service> {
        self.services
            .iter_mut()
            .find(|svc| svc.service_id == service_id)
    }
}

impl isdb::demux::Filter for Filter {
    type Tag = Tag;

    fn on_pes_packet(&mut self, _: &mut isdb::demux::Context<Tag>, _: &isdb::pes::PesPacket) {}

    fn on_setup(&mut self, table: &mut isdb::demux::Table<Tag>) {
        table.set_as_psi(Pid::PAT, Tag::Pat);
        table.set_as_psi(Pid::SDT, Tag::Sdt);
        table.set_as_psi(Pid::H_EIT, Tag::Eit);
        table.set_as_psi(Pid::M_EIT, Tag::Eit);
        table.set_as_psi(Pid::L_EIT, Tag::Eit);
    }

    fn on_psi_section(&mut self, ctx: &mut isdb::demux::Context<Tag>, psi: &isdb::psi::PsiSection) {
        match ctx.tag() {
            Tag::Pat => {
                let Some(pat) = self.repo.read::<isdb::psi::table::Pat>(psi) else {
                    return;
                };

                for program in &*pat.pmts {
                    if self.find_service(program.program_number).is_some() {
                        continue;
                    }

                    self.services.push(Service {
                        service_id: program.program_number,
                        provider_name: AribString::new(),
                        service_name: AribString::new(),
                        events: Vec::new(),
                    });
                }
            }

            Tag::Sdt => {
                let sdt = match self.repo.read::<isdb::psi::table::Sdt>(psi) {
                    Some(isdb::psi::table::Sdt::Actual(sdt)) => sdt,
                    _ => return,
                };

                for svc in &*sdt.services {
                    let Some(service) = self.find_service(svc.service_id) else {
                        continue;
                    };
                    let Some(sd) = svc.descriptors.get::<isdb::psi::desc::ServiceDescriptor>()
                    else {
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

            Tag::Eit => {
                let eit = match self.repo.read::<isdb::psi::table::Eit>(psi) {
                    Some(isdb::psi::table::Eit::ActualPf(eit)) => eit,
                    _ => return,
                };
                // 現在のイベントのみ
                if eit.section_number != 0 {
                    return;
                }
                let Some(service) = self.find_service(eit.service_id) else {
                    self.repo.unset(psi);
                    return;
                };
                let Some(event) = eit.events.first() else {
                    return;
                };
                if service.has_event(event.event_id) {
                    return;
                }
                let Some(sed) = event
                    .descriptors
                    .get::<isdb::psi::desc::ShortEventDescriptor>()
                else {
                    return;
                };

                service.events.push(Event {
                    event_id: event.event_id,
                    name: sed.event_name.to_owned(),
                });
            }
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
        demuxer.feed(&packet?);
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
