use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use fxhash::{FxHashMap, FxHashSet};
use isdb::psi::table::ServiceId;
use isdb::Pid;

#[derive(Debug)]
struct AppArgs {
    input: PathBuf,
    output: Option<PathBuf>,
}

impl AppArgs {
    const HELP: &str = "\
字幕や文字スーパーからDRCSを抽出して表示・保存するコマンド

USAGE:
  drcs [OPTIONS] [PATH]

FLAGS:
  -h, --help     このヘルプを表示する

OPTIONS:
  --output PATH  DRCSをPNGとして出力するディレクトリ

ARGS:
  <PATH>         DRCSを表示するTSファイルのパス
";

    pub fn parse() -> Result<AppArgs, Box<dyn std::error::Error>> {
        let mut args = pico_args::Arguments::from_env();

        if args.contains(["-h", "--help"]) {
            println!("{}", Self::HELP);
            std::process::exit(0);
        }

        let output = args.opt_value_from_str("--output")?;

        Ok(AppArgs {
            input: args.free_from_str()?,
            output,
        })
    }
}

#[derive(Debug)]
struct Service {
    service_id: ServiceId,
    pmt_pid: Pid,
    pcr_pid: Pid,
    caption_pids: Vec<Pid>,

    last_pcr: Option<isdb::time::Timestamp>,
    base_pcr: Option<isdb::time::Timestamp>,

    patterns: FxHashSet<Vec<u8>>,
}

impl Service {
    pub fn pcr_diff(&self) -> Option<chrono::Duration> {
        match (self.last_pcr, self.base_pcr) {
            (Some(last_pcr), Some(base_pcr)) => Some(chrono::Duration::nanoseconds(
                (last_pcr - base_pcr).as_nanos() as i64,
            )),
            _ => None,
        }
    }
}

struct Filter {
    output: Option<PathBuf>,

    repo: isdb::psi::Repository,
    services: FxHashMap<ServiceId, Service>,
    current_time: Option<chrono::NaiveDateTime>,
}

impl Filter {
    pub fn new(output: Option<PathBuf>) -> Filter {
        Filter {
            output,

            repo: isdb::psi::Repository::new(),
            services: FxHashMap::default(),
            current_time: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tag {
    Pat,
    Pmt,
    Tot,
    Pcr,
    Caption,
}

impl isdb::demux::Filter for Filter {
    type Tag = Tag;

    fn on_setup(&mut self, table: &mut isdb::demux::Table<Tag>) {
        table.set_as_psi(Pid::PAT, Tag::Pat);
        table.set_as_psi(Pid::TOT, Tag::Tot);
    }

    fn on_packet_storing(&mut self, ctx: &mut isdb::demux::Context<Tag>) {
        let Some(pcr) = ctx.packet().adaptation_field().and_then(|af| af.pcr()) else {
            return;
        };

        let pid = ctx.packet().pid();
        for service in self.services.values_mut() {
            if service.pcr_pid == pid {
                service.last_pcr = Some(pcr);
            }
        }
    }

    fn on_psi_section(&mut self, ctx: &mut isdb::demux::Context<Tag>, psi: &isdb::psi::PsiSection) {
        match ctx.tag() {
            Tag::Pat => {
                let Some(pat) = self.repo.read::<isdb::psi::table::Pat>(psi) else {
                    return;
                };

                for service in self.services.values() {
                    ctx.table().unset(service.pmt_pid);
                    ctx.table().unset(service.pcr_pid);
                    for &pid in &service.caption_pids {
                        ctx.table().unset(pid);
                    }
                }

                self.services.clear();
                for program in &*pat.pmts {
                    self.services.insert(
                        program.program_number,
                        Service {
                            service_id: program.program_number,
                            pmt_pid: program.program_map_pid,
                            pcr_pid: Pid::NULL,
                            caption_pids: Vec::new(),
                            last_pcr: None,
                            base_pcr: None,
                            patterns: FxHashSet::default(),
                        },
                    );

                    ctx.table().set_as_psi(program.program_map_pid, Tag::Pmt);
                }
            }

            Tag::Pmt => {
                let Some(pmt) = self.repo.read::<isdb::psi::table::Pmt>(psi) else {
                    return;
                };
                let Some(service) = self.services.get_mut(&pmt.program_number) else {
                    return;
                };

                if service.pcr_pid != pmt.pcr_pid {
                    if pmt.pcr_pid != Pid::NULL {
                        ctx.table().set_as_custom(pmt.pcr_pid, Tag::Pcr);
                    } else {
                        ctx.table().unset(service.pcr_pid);
                    }
                    service.pcr_pid = pmt.pcr_pid;
                }
                for &pid in &*service.caption_pids {
                    ctx.table().unset(pid);
                }

                for stream in &*pmt.streams {
                    if stream.stream_type != isdb::psi::desc::StreamType::CAPTION {
                        continue;
                    }

                    ctx.table().set_as_pes(stream.elementary_pid, Tag::Caption);
                    service.caption_pids.push(stream.elementary_pid);
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
                for service in self.services.values_mut() {
                    service.base_pcr = service.last_pcr;
                }
            }
            Tag::Pcr | Tag::Caption => {
                log::error!("来るはずのないタグ：{:?}", ctx.tag());
            }
        }
    }

    fn on_pes_packet(&mut self, ctx: &mut isdb::demux::Context<Tag>, pes: &isdb::pes::PesPacket) {
        if !matches!(ctx.tag(), Tag::Caption) {
            log::error!("来るはずのないタグ：{:?}", ctx.tag());
            return;
        };

        let Some(service) = self
            .services
            .values_mut()
            .find(|svc| svc.caption_pids.contains(&ctx.packet().pid()))
        else {
            log::error!("サービスがない：{:?}", ctx.packet().pid());
            return;
        };

        let Some(current) = self
            .current_time
            .and_then(|current_time| Some(current_time + service.pcr_diff()?))
        else {
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

        for unit in data_units {
            use isdb::pes::caption::DataUnit;
            let (DataUnit::DrcsSb(drcs) | DataUnit::DrcsDb(drcs)) = unit else { continue };

            for code in &*drcs.codes {
                for (i, font) in code.fonts.iter().enumerate() {
                    let Some(data) = font.data.uncompressed() else { continue };

                    // ヒープの無駄な使用を抑えるためにcontainsとinsertを分ける
                    if service.patterns.contains(data.pattern_data) {
                        // 既知のパターン
                        continue;
                    }
                    service.patterns.insert(data.pattern_data.to_vec());

                    // 再利用しやすいようにビットマップを作ってから表示する
                    let bpp = match data.depth {
                        0 => 1,
                        2 => 2,
                        _ => {
                            log::warn!("知らない階調だ・・・");
                            continue;
                        }
                    };
                    let mask = (1 << bpp) - 1;
                    let size = (data.width as usize) * (data.height as usize);
                    let bitmap: Vec<f32> = (0..size * bpp)
                        .step_by(bpp)
                        .map(|p| {
                            let pos = p / 8;
                            let shift = 8 - bpp - p % 8;
                            let bits = (data.pattern_data[pos] >> shift) & mask;
                            (bits as f32) / (mask as f32)
                        })
                        .collect();
                    assert_eq!(bitmap.len(), size);

                    if let Some(output) = &self.output {
                        let path = output.join(format!(
                            "{:04X}_{:04X}_{}_{}.png",
                            service.service_id,
                            code.character_code,
                            i + 1,
                            service.patterns.len(),
                        ));

                        let image = image::ImageBuffer::from_fn(
                            data.width as u32,
                            data.height as u32,
                            |x, y| {
                                let p = (x + y * data.width as u32) as usize;
                                image::LumaA([0, (bitmap[p] * 255.) as u8])
                            },
                        );
                        match image.save_with_format(&*path, image::ImageFormat::Png) {
                            Ok(()) => println!("'{}'へDRCSを保存", path.display()),
                            Err(e) => {
                                log::error!("'{}'へのロゴの保存に失敗：{}", path.display(), e);
                            }
                        }
                    } else {
                        println!(
                            "{} - {}x{} ({:04X} - {:04X}[{}])",
                            current.format("%F %T%.3f"),
                            data.width,
                            data.height,
                            service.service_id,
                            code.character_code,
                            i + 1,
                        );
                        for p in (0..size).step_by(data.width as usize) {
                            for x in 0..data.width as usize {
                                let s = if bitmap[p + x] > 0. { "■" } else { "□" };
                                print!("{}", s);
                            }
                            println!();
                        }
                        println!();
                    }
                }
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = AppArgs::parse()?;

    env_logger::init();

    let f = File::open(&*args.input)?;
    let f = BufReader::with_capacity(188 * 1024, f);

    let mut demuxer = isdb::demux::Demuxer::new(Filter::new(args.output));

    for packet in isdb::Packet::iter(f) {
        demuxer.feed(&packet?);
    }

    Ok(())
}
