use std::fs::File;
use std::io::BufReader;

use fxhash::FxHashMap;
use isdb::psi::table::{EventId, NetworkId, ServiceId};
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
    events: FxHashMap<EventId, Event>,
}

#[derive(Debug, Default)]
struct Network {
    services: FxHashMap<ServiceId, Service>,
}

struct Filter {
    repo: isdb::psi::Repository,
    networks: FxHashMap<NetworkId, Network>,
}

impl Filter {
    pub fn new() -> Filter {
        Filter {
            repo: isdb::psi::Repository::new(),
            networks: FxHashMap::default(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Tag {
    Sdt,
    Eit,
}

impl isdb::demux::Filter for Filter {
    type Tag = Tag;

    fn on_pes_packet(&mut self, _: &mut isdb::demux::Context<Tag>, _: &isdb::pes::PesPacket) {}

    fn on_setup(&mut self, table: &mut isdb::demux::Table<Tag>) {
        table.set_as_psi(Pid::SDT, Tag::Sdt);
        table.set_as_psi(Pid::H_EIT, Tag::Eit);
    }

    fn on_psi_section(&mut self, ctx: &mut isdb::demux::Context<Tag>, psi: &isdb::psi::PsiSection) {
        match ctx.tag() {
            Tag::Sdt => {
                let sdt = match self.repo.read::<isdb::psi::table::Sdt>(psi) {
                    Some(isdb::psi::table::Sdt::Actual(sdt)) => sdt,
                    Some(isdb::psi::table::Sdt::Other(sdt)) => sdt,
                    None => {
                        return;
                    }
                };

                let network = self
                    .networks
                    .entry(sdt.original_network_id)
                    .or_insert_with(Default::default);
                for service in &*sdt.services {
                    let Some(svc) = service.descriptors.get::<isdb::psi::desc::ServiceDescriptor>()
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
            Tag::Eit => {
                let eit = match self.repo.read::<isdb::psi::table::Eit>(psi) {
                    Some(isdb::psi::table::Eit::ActualSchedule(eit)) => eit,
                    Some(isdb::psi::table::Eit::OtherSchedule(eit)) => eit,
                    Some(_) => return,
                    None => {
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

                    if let Some(short_event) = ev
                        .descriptors
                        .get::<isdb::psi::desc::ShortEventDescriptor>()
                    {
                        event.name = short_event.event_name.to_string(Default::default());
                        event.short = short_event.text.to_string(Default::default());
                    }

                    let mut items = Vec::new();
                    for item in ev
                        .descriptors
                        .get_all::<isdb::psi::desc::ExtendedEventDescriptor>()
                        .flat_map(|extended_event| extended_event.items)
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
                    for line in s.split('\n') {
                        println!("\t{}", line);
                    }
                }
                println!("\t{}", "-".repeat(100));
            }
        }
    }

    Ok(())
}
