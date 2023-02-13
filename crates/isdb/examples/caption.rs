use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use fxhash::FxHashMap;
use isdb::Pid;

#[derive(Debug)]
struct AppArgs {
    service: Option<u16>,
    path: PathBuf,
}

impl AppArgs {
    const HELP: &str = "\
字幕を表示するコマンド

USAGE:
  caption [OPTIONS] [PATH]

FLAGS:
  -h, --help    このヘルプを表示する

OPTIONS:
  --sid [SID]   表示する字幕のサービスID。
                未指定の場合はデフォルトのサービスが選択される。

ARGS:
  <PATH>        字幕を表示するTSファイルのパス
";

    pub fn parse() -> Result<AppArgs, Box<dyn std::error::Error>> {
        let mut args = pico_args::Arguments::from_env();

        if args.contains(["-h", "--help"]) {
            println!("{}", Self::HELP);
            std::process::exit(0);
        }

        let service = args.opt_value_from_str("--sid")?;

        Ok(AppArgs {
            service,
            path: args.free_from_str()?,
        })
    }
}

struct CaptionEs {
    is_oneseg: bool,
}

struct Filter {
    manual_service_id: Option<u16>,

    current_service_id: Option<u16>,
    pmt_pid: Pid,
    caption_es: FxHashMap<Pid, CaptionEs>,

    pcr_pid: Pid,
    last_pcr: Option<chrono::Duration>,
    base_pcr: Option<chrono::Duration>,
    current_time: Option<chrono::NaiveDateTime>,
}

impl Filter {
    pub fn new(service_id: Option<u16>) -> Filter {
        Filter {
            manual_service_id: service_id,

            current_service_id: None,
            pmt_pid: Pid::NULL,
            caption_es: FxHashMap::default(),

            pcr_pid: Pid::NULL,
            last_pcr: None,
            base_pcr: None,
            current_time: None,
        }
    }

    pub fn current(&self) -> Option<chrono::NaiveDateTime> {
        match (self.last_pcr, self.base_pcr, self.current_time) {
            (Some(last_pcr), Some(base_pcr), Some(current_time)) => {
                Some(current_time + (last_pcr - base_pcr))
            }
            _ => None,
        }
    }
}

impl isdb::demux::Filter for Filter {
    fn on_packet(&mut self, packet: &isdb::Packet) -> Option<isdb::demux::PacketType> {
        if packet.pid() == self.pcr_pid {
            if let Some(pcr) = packet.adaptation_field().and_then(|af| af.pcr) {
                self.last_pcr = Some(chrono::Duration::nanoseconds(pcr.to_nanos() as i64));
            }
        }

        match packet.pid() {
            Pid::PAT | Pid::TOT => Some(isdb::demux::PacketType::Psi),
            pid if self.pmt_pid == pid => Some(isdb::demux::PacketType::Psi),
            pid if self.caption_es.contains_key(&pid) => Some(isdb::demux::PacketType::Pes),
            _ => None,
        }
    }

    fn on_psi_section(&mut self, packet: &isdb::Packet, psi: &isdb::psi::PsiSection) {
        match packet.pid() {
            Pid::PAT => {
                let Some(pat) = isdb::table::Pat::read(psi) else {
                    return;
                };

                self.pmt_pid = Pid::NULL;
                self.current_service_id = None;
                let program = match self.manual_service_id {
                    // サービスIDが指定されていない場合は最初のサービスが対象
                    None => pat.pmts.first(),

                    // サービスIDが指定されている場合はそのサービスを使用
                    Some(service_id) => pat
                        .pmts
                        .iter()
                        .find(|program| program.program_number.get() == service_id),
                };
                let Some(program) = program else { return };

                self.pmt_pid = program.program_map_pid;
                self.current_service_id = Some(program.program_number.get());
            }

            pid if self.pmt_pid == pid => {
                let Some(service_id) = self.current_service_id else {
                    return;
                };
                let Some(pmt) = isdb::table::Pmt::read(psi) else {
                    return;
                };
                if pmt.program_number != service_id {
                    return;
                }

                self.pcr_pid = pmt.pcr_pid;
                self.caption_es.clear();

                for stream in &*pmt.streams {
                    if stream.stream_type != isdb::desc::StreamType::CAPTION {
                        continue;
                    }

                    // let component_tag = stream
                    //     .descriptors
                    //     .get::<isdb::desc::StreamIdDescriptor>()
                    //     .map(|desc| desc.component_tag);

                    self.caption_es.insert(
                        stream.elementary_pid,
                        CaptionEs {
                            is_oneseg: Pid::ONESEG_PMT_PID.contains(&pid),
                        },
                    );
                }
            }

            Pid::TOT => {
                let Some(tot) = isdb::table::Tot::read(psi) else {
                    return;
                };

                let Some(date) = tot.jst_time.date.to_date() else {
                    return;
                };
                let Some(date) = chrono::NaiveDate::from_ymd_opt(
                    date.year,
                    date.month as u32,
                    date.day as u32
                ) else {
                    return;
                };
                let Some(time) = chrono::NaiveTime::from_hms_opt(
                    tot.jst_time.hour as u32,
                    tot.jst_time.minute as u32,
                    tot.jst_time.second as u32,
                ) else {
                    return;
                };
                let dt = chrono::NaiveDateTime::new(date, time);

                self.current_time = Some(dt);
                self.base_pcr = self.last_pcr;
            }

            _ => {}
        }
    }

    fn on_pes_packet(&mut self, packet: &isdb::Packet, pes: &isdb::pes::PesPacket) {
        let Some(current) = self.current() else {
            return;
        };
        let Some(caption_es) = self.caption_es.get(&packet.pid()) else {
            return;
        };
        let Some(pes) = isdb::pes::IndependentPes::read(pes.data) else {
            return;
        };
        let Some(data_group) = isdb::caption::DataGroup::read(pes.data().pes_data) else {
            return;
        };

        let data_units = if matches!(data_group.data_group_id, 0x00 | 0x20) {
            use isdb::caption::CaptionManagementData;
            let Some(management) = CaptionManagementData::read(data_group.data_group_data) else {
                return;
            };

            management.data_units
        } else {
            let Some(caption) = isdb::caption::CaptionData::read(data_group.data_group_data) else {
                return;
            };

            caption.data_units
        };

        let decode_opts = if caption_es.is_oneseg {
            isdb::eight::decode::Options::ONESEG_CAPTION
        } else {
            isdb::eight::decode::Options::CAPTION
        };

        for unit in data_units {
            let isdb::caption::DataUnit::StatementBody(caption) = unit else {
                continue;
            };

            if log::log_enabled!(log::Level::Debug) {
                for c in caption.decode(decode_opts) {
                    log::debug!("{:?}", c);
                }
            }
            let caption = caption.to_string(decode_opts);
            if !caption.is_empty() {
                println!("{} - {}", current.format("%F %T%.3f"), caption);
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = AppArgs::parse()?;

    env_logger::init();

    let f = File::open(&*args.path)?;
    let f = BufReader::with_capacity(188 * 1024, f);

    let mut demuxer = isdb::demux::Demuxer::new(Filter::new(args.service));

    for packet in isdb::Packet::iter(f) {
        demuxer.feed(&packet?);
    }

    Ok(())
}
