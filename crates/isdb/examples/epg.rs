use std::fs::File;
use std::io::BufReader;

use fxhash::FxHashMap;
use isdb::Pid;

#[derive(Debug)]
struct Event {
    /// 番組開始時刻。
    start_dt: chrono::NaiveDateTime,
    /// 番組の継続時間。
    duration: chrono::Duration,
    /// 番組名。
    name: String,
    /// 短形式イベント。
    short: String,
    /// 拡張形式イベント。
    extended: Vec<(String, String)>,
}

#[derive(Debug, Default)]
struct Service {
    /// サービス名。
    name: String,
    /// キーはイベント識別。
    events: FxHashMap<u16, Event>,
}

#[derive(Debug, Default)]
struct Network {
    /// キーはサービス識別。
    services: FxHashMap<u16, Service>,
}

struct Filter {
    /// キーはネットワーク識別。
    networks: FxHashMap<u16, Network>,
}

impl Filter {
    pub fn new() -> Filter {
        Filter {
            networks: FxHashMap::default(),
        }
    }
}

impl isdb::demux::Filter for Filter {
    fn on_pes_packet(&mut self, _: &isdb::Packet, _: &isdb::pes::PesPacket) {}

    fn on_packet(&mut self, packet: &isdb::Packet) -> Option<isdb::demux::PacketType> {
        match packet.pid() {
            Pid::SDT | Pid::H_EIT => Some(isdb::demux::PacketType::Psi),
            _ => None,
        }
    }

    fn on_psi_section(&mut self, packet: &isdb::Packet, psi: &isdb::psi::PsiSection) {
        match packet.pid() {
            Pid::SDT => {
                let sdt = match isdb::table::Sdt::read(psi) {
                    Some(isdb::table::Sdt::Actual(sdt)) => sdt,
                    Some(isdb::table::Sdt::Other(sdt)) => sdt,
                    None => {
                        log::warn!("invalid SDT");
                        return;
                    }
                };

                let network = self
                    .networks
                    .entry(sdt.original_network_id)
                    .or_insert_with(Default::default);
                for service in &*sdt.services {
                    let Some(svc) = service.descriptors.get::<isdb::desc::ServiceDescriptor>()
                    else {
                        continue;
                    };

                    if !svc.service_name.is_empty() {
                        match network.services.entry(service.service_id) {
                            std::collections::hash_map::Entry::Occupied(mut entry) => {
                                entry.get_mut().name =
                                    svc.service_name.to_string(Default::default());
                            }
                            std::collections::hash_map::Entry::Vacant(entry) => {
                                entry.insert(Service {
                                    name: svc.service_name.to_string(Default::default()),
                                    events: Default::default(),
                                });
                            }
                        }
                    }
                }
            }
            Pid::H_EIT => {
                let eit = match isdb::table::Eit::read(psi) {
                    Some(isdb::table::Eit::ActualSchedule(eit)) => eit,
                    Some(isdb::table::Eit::OtherSchedule(eit)) => eit,
                    Some(_) => return,
                    None => {
                        log::warn!("invalid EIT");
                        return;
                    }
                };

                let Some(network) = self.networks.get_mut(&eit.original_network_id) else {
                    return;
                };
                let service = network
                    .services
                    .entry(eit.service_id)
                    .or_insert_with(Default::default);

                for ev in &*eit.events {
                    let Some(date) = ev.start_time.date.to_date() else {
                        continue;
                    };
                    let Some(date) = chrono::NaiveDate::from_ymd_opt(
                        date.year,
                        date.month as u32,
                        date.day as u32
                    ) else {
                        continue;
                    };
                    let Some(time) = chrono::NaiveTime::from_hms_opt(
                        ev.start_time.hour as u32,
                        ev.start_time.minute as u32,
                        ev.start_time.second as u32,
                    ) else {
                        continue;
                    };

                    let start_dt = chrono::NaiveDateTime::new(date, time);
                    let duration = chrono::Duration::seconds(ev.duration as i64);

                    let event = service.events.entry(ev.event_id).or_insert_with(|| Event {
                        start_dt,
                        duration,
                        name: String::new(),
                        short: String::new(),
                        extended: Vec::new(),
                    });

                    if let Some(short_event) =
                        ev.descriptors.get::<isdb::desc::ShortEventDescriptor>()
                    {
                        event.name = short_event.event_name.to_string(Default::default());
                        event.short = short_event.text.to_string(Default::default());
                    }

                    let mut items = Vec::new();
                    for item in ev
                        .descriptors
                        .get_all::<isdb::desc::ExtendedEventDescriptor>()
                        .flat_map(|extended_event| extended_event.items.into_iter())
                    {
                        match (item.item_description.is_empty(), items.last_mut()) {
                            (false, _) | (true, None) => {
                                // 項目名がある、または最初の項目なので新規追加
                                items.push((
                                    item.item_description.to_string(Default::default()),
                                    item.item.to_owned(),
                                ));
                            }
                            (true, Some(last_item)) => {
                                // 項目名がないので項目継続
                                last_item.1.push_str(item.item);
                            }
                        }
                    }
                    if !items.is_empty() {
                        event.extended = items
                            .into_iter()
                            .map(|(k, v)| (k, v.to_string(Default::default())))
                            .collect();
                    }
                }
            }
            _ => {}
        }
    }
}

const HELP: &str = "\
番組表を表示するコマンド

USAGE:
  epg [PATH]

FLAGS:
  -h, --help このヘルプを表示する

ARGS:
  <PATH>     番組情報を表示するTSファイルのパス
";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = pico_args::Arguments::from_env();
    if args.contains(["-h", "--help"]) {
        print!("{}", HELP);
        std::process::exit(0);
    }

    let path: std::path::PathBuf = args.free_from_str()?;

    env_logger::init();

    let f = File::open(path)?;
    let f = BufReader::with_capacity(188 * 1024, f);

    let mut demuxer = isdb::demux::Demuxer::new(Filter::new());

    for packet in isdb::Packet::iter(f) {
        demuxer.feed(&packet?);
    }

    let networks = demuxer.into_filter().networks;
    for network in networks.values() {
        for service in network.services.values() {
            let mut events: Vec<&Event> = service
                .events
                .values()
                .filter(|ev| !ev.name.is_empty())
                .collect();
            if events.is_empty() {
                continue;
            }
            events.sort_unstable_by_key(|e| e.start_dt);

            println!("{}", service.name);

            for event in events {
                println!(
                    "\t{} - {} {}",
                    event.start_dt,
                    event.start_dt + event.duration,
                    event.name,
                );
                if !event.short.is_empty() {
                    println!("\t{}", event.short);
                    if !event.extended.is_empty() {
                        println!();
                    }
                }
                for (i, (_, s)) in event.extended.iter().enumerate() {
                    if i != 0 {
                        println!();
                    }
                    println!("\t{}", s);
                }
                println!("\t{}", "-".repeat(100));
            }
        }
    }

    Ok(())
}
