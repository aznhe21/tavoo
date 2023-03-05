use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use isdb::psi::table::ServiceId;
use isdb::Pid;

#[derive(Debug)]
struct AppArgs {
    service: Option<ServiceId>,
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

        let service = args.opt_value_from_str("--sid")?.and_then(ServiceId::new);

        Ok(AppArgs {
            service,
            path: args.free_from_str()?,
        })
    }
}

struct Filter {
    manual_service_id: Option<ServiceId>,

    repo: isdb::psi::Repository,
    current_service_id: Option<ServiceId>,
    pmt_pid: Pid,
    caption_pids: Vec<Pid>,

    pcr_pid: Pid,
    last_pcr: Option<chrono::Duration>,
    base_pcr: Option<chrono::Duration>,
    current_time: Option<chrono::NaiveDateTime>,
}

impl Filter {
    pub fn new(service_id: Option<ServiceId>) -> Filter {
        Filter {
            manual_service_id: service_id,

            repo: isdb::psi::Repository::new(),
            current_service_id: None,
            pmt_pid: Pid::NULL,
            caption_pids: Vec::new(),

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tag {
    Pat,
    Pmt,
    Tot,
    Pcr,
    Caption(bool),
}

impl isdb::demux::Filter for Filter {
    type Tag = Tag;

    fn on_setup(&mut self, table: &mut isdb::demux::Table<Tag>) {
        table.set_as_psi(Pid::PAT, Tag::Pat);
        table.set_as_psi(Pid::TOT, Tag::Tot);
    }

    fn on_psi_section(&mut self, ctx: &mut isdb::demux::Context<Tag>, psi: &isdb::psi::PsiSection) {
        match ctx.tag() {
            Tag::Pat => {
                let Some(pat) = self.repo.read::<isdb::psi::table::Pat>(psi) else {
                    return;
                };

                ctx.table().unset(self.pmt_pid);
                self.pmt_pid = Pid::NULL;
                self.current_service_id = None;

                let program = match self.manual_service_id {
                    // サービスIDが指定されていない場合は最初のサービスが対象
                    None => pat.pmts.first(),

                    // サービスIDが指定されている場合はそのサービスを使用
                    Some(service_id) => pat
                        .pmts
                        .iter()
                        .find(|program| program.program_number == service_id),
                };
                let Some(program) = program else { return };

                self.pmt_pid = program.program_map_pid;
                ctx.table().set_as_psi(self.pmt_pid, Tag::Pmt);
                self.current_service_id = Some(program.program_number);
            }

            Tag::Pmt => {
                let Some(service_id) = self.current_service_id else {
                    return;
                };
                let Some(pmt) = self.repo.read::<isdb::psi::table::Pmt>(psi) else {
                    return;
                };
                if pmt.program_number != service_id {
                    return;
                }

                if self.pcr_pid != pmt.pcr_pid {
                    ctx.table().unset(self.pcr_pid);
                    self.pcr_pid = pmt.pcr_pid;
                    ctx.table().set_as_custom(pmt.pcr_pid, Tag::Pcr);
                }
                for pid in self.caption_pids.drain(..) {
                    ctx.table().unset(pid);
                }

                for stream in &*pmt.streams {
                    if stream.stream_type != isdb::psi::desc::StreamType::CAPTION {
                        continue;
                    }

                    // let component_tag = stream
                    //     .descriptors
                    //     .get::<isdb::desc::StreamIdDescriptor>()
                    //     .map(|desc| desc.component_tag);

                    self.caption_pids.push(stream.elementary_pid);
                    let is_oneseg = ctx.packet().pid().is_oneseg_pmt();
                    ctx.table()
                        .set_as_pes(stream.elementary_pid, Tag::Caption(is_oneseg));
                }
            }

            Tag::Tot => {
                let Some(tot) = self.repo.read::<isdb::psi::table::Tot>(psi) else {
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
            Tag::Pcr | Tag::Caption(_) => {
                log::error!("来るはずのないタグ：{:?}", ctx.tag());
            }
        }
    }

    fn on_pes_packet(&mut self, ctx: &mut isdb::demux::Context<Tag>, pes: &isdb::pes::PesPacket) {
        let Tag::Caption(is_oneseg) = ctx.tag() else {
            log::error!("来るはずのないタグ：{:?}", ctx.tag());
            return;
        };

        let Some(current) = self.current() else {
            return;
        };
        let Some(pes) = isdb::pes::IndependentPes::read(pes.data) else {
            return;
        };
        let Some(data_group) = isdb::pes::caption::DataGroup::read(pes.data().pes_data) else {
            return;
        };

        let data_units = if matches!(data_group.data_group_id, 0x00 | 0x20) {
            use isdb::pes::caption::CaptionManagementData;
            let Some(management) = CaptionManagementData::read(data_group.data_group_data) else {
                return;
            };

            management.data_units
        } else {
            let Some(caption) = isdb::pes::caption::CaptionData::read(data_group.data_group_data) else {
                return;
            };

            caption.data_units
        };

        let decode_opts = if is_oneseg {
            isdb::eight::decode::Options::ONESEG_CAPTION
        } else {
            isdb::eight::decode::Options::CAPTION
        };

        for unit in data_units {
            let isdb::pes::caption::DataUnit::StatementBody(caption) = unit else {
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

    fn on_custom_packet(&mut self, ctx: &mut isdb::demux::Context<Tag>, _: bool) {
        let Some(af) = ctx.packet().adaptation_field() else { return };
        let Some(pcr) = af.pcr() else { return };

        self.last_pcr = Some(chrono::Duration::nanoseconds(pcr.as_nanos() as i64));
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
