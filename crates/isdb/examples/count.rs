use std::fs::File;
use std::io::BufReader;

use fxhash::{FxHashMap, FxHashSet};
use isdb::Pid;

#[derive(Default)]
struct Count {
    input: u64,
    continuity_error: u64,
    scrambled: u64,
}

struct Counter {
    input: u64,
    format_error: u64,
    transport_error: u64,
    counts: isdb::pid::PidTable<Count>,
}

#[derive(Default)]
struct Service {
    pmt_pids: FxHashSet<Pid>,
    pcr_pids: FxHashSet<Pid>,
    ecm_pids: FxHashSet<Pid>,
    stream_types: FxHashMap<Pid, isdb::psi::desc::StreamType>,
}

struct Filter {
    repo: isdb::psi::Repository,
    current_pmt_pids: Vec<Pid>,

    services: FxHashMap<u16, Service>,
    emm_pids: FxHashSet<Pid>,

    counter: Counter,
}

impl Filter {
    pub fn new() -> Filter {
        Filter {
            repo: isdb::psi::Repository::new(),
            current_pmt_pids: Vec::new(),

            services: FxHashMap::default(),
            emm_pids: FxHashSet::default(),

            counter: Counter {
                input: 0,
                format_error: 0,
                transport_error: 0,
                counts: isdb::pid::PidTable::from_fn(|_| Count::default()),
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Tag {
    Pat,
    Pmt,
    Cat,
}

impl isdb::demux::Filter for Filter {
    type Tag = Tag;

    fn on_pes_packet(&mut self, _: &mut isdb::demux::Context<Tag>, _: &isdb::pes::PesPacket) {}

    fn on_setup(&mut self, table: &mut isdb::demux::Table<Tag>) {
        table.set_as_psi(Pid::PAT, Tag::Pat);
        table.set_as_psi(Pid::CAT, Tag::Cat);
    }

    fn on_discontinued(&mut self, packet: &isdb::Packet) {
        let mut count = &mut self.counter.counts[packet.pid()];
        count.continuity_error += 1;
    }

    fn on_psi_section(&mut self, ctx: &mut isdb::demux::Context<Tag>, psi: &isdb::psi::PsiSection) {
        match ctx.tag() {
            Tag::Pat => {
                let Some(pat) = self.repo.read::<isdb::psi::table::Pat>(psi) else {
                    return;
                };

                for pid in self.current_pmt_pids.drain(..) {
                    ctx.table().unset(pid);
                }

                for program in &*pat.pmts {
                    self.current_pmt_pids.push(program.program_map_pid);

                    let service = self
                        .services
                        .entry(program.program_number.get())
                        .or_insert_with(Default::default);
                    service.pmt_pids.insert(program.program_map_pid);
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

                service.pcr_pids.insert(pmt.pcr_pid);

                for stream in &*pmt.streams {
                    service
                        .stream_types
                        .insert(stream.elementary_pid, stream.stream_type);
                }

                for cad in pmt
                    .descriptors
                    .get_all::<isdb::psi::desc::ConditionalAccessDescriptor>()
                {
                    service.ecm_pids.insert(cad.ca_pid);
                }
            }
            Tag::Cat => {
                let Some(cat) = self.repo.read::<isdb::psi::table::Cat>(psi) else {
                    return;
                };

                for cad in cat
                    .descriptors
                    .get_all::<isdb::psi::desc::ConditionalAccessDescriptor>()
                {
                    self.emm_pids.insert(cad.ca_pid);
                }
            }
        }
    }
}

fn pid_description(pid: Pid) -> Option<&'static str> {
    match pid {
        Pid::PAT => Some("PAT"),
        Pid::CAT => Some("CAT"),
        Pid::NIT => Some("NIT"),
        Pid::SDT => Some("SDT"),
        Pid::H_EIT => Some("H-EIT"),
        Pid::TOT => Some("TOT"),
        Pid::SDTT => Some("SDTT"),
        Pid::BIT => Some("BIT"),
        Pid::NBIT => Some("NBIT"),
        Pid::M_EIT => Some("M-EIT"),
        Pid::L_EIT => Some("L-EIT"),
        Pid::CDT => Some("CDT"),
        Pid::NULL => Some("Null"),
        _ => None,
    }
}

fn stream_type_description(stream_type: isdb::psi::desc::StreamType) -> Option<&'static str> {
    use isdb::psi::desc::StreamType;
    match stream_type {
        StreamType::MPEG1_VIDEO => Some("MPEG-1 Video"),
        StreamType::MPEG2_VIDEO => Some("MPEG-2 Video"),
        StreamType::MPEG1_AUDIO => Some("MPEG-1 Audio"),
        StreamType::MPEG2_AUDIO => Some("MPEG-2 Audio"),
        StreamType::PRIVATE_SECTIONS => Some("private_sections"),
        StreamType::PRIVATE_DATA => Some("private data"),
        StreamType::MHEG => Some("MHEG"),
        StreamType::DSM_CC => Some("DSM-CC"),
        StreamType::ITU_T_REC_H222_1 => Some("H.222.1"),
        StreamType::ISO_IEC_13818_6_TYPE_A => Some("ISO/IEC 13818-6 type A"),
        StreamType::ISO_IEC_13818_6_TYPE_B => Some("ISO/IEC 13818-6 type B"),
        StreamType::ISO_IEC_13818_6_TYPE_C => Some("ISO/IEC 13818-6 type C"),
        StreamType::ISO_IEC_13818_6_TYPE_D => Some("ISO/IEC 13818-6 type D"),
        StreamType::ISO_IEC_13818_1_AUXILIARY => Some("auxiliary"),
        StreamType::AAC => Some("AAC"),
        StreamType::MPEG4_VISUAL => Some("MPEG-4 Visual"),
        StreamType::MPEG4_AUDIO => Some("MPEG-4 Audio"),
        StreamType::ISO_IEC_14496_1_IN_PES => Some("ISO/IEC 14496-1 in PES packets"),
        StreamType::ISO_IEC_14496_1_IN_SECTIONS => {
            Some("ISO/IEC 14496-1 in ISO/IEC 14496_sections")
        }
        StreamType::ISO_IEC_13818_6_DOWNLOAD => {
            Some("ISO/IEC 13818-6 Synchronized Download Protocol")
        }
        StreamType::METADATA_IN_PES => Some("Metadata in PES packets"),
        StreamType::METADATA_IN_SECTIONS => Some("Metadata in metadata_sections"),
        StreamType::METADATA_IN_DATA_CAROUSEL => Some("Metadata in ISO/IEC 13818-6 Data Carousel"),
        StreamType::METADATA_IN_OBJECT_CAROUSEL => {
            Some("Metadata in ISO/IEC 13818-6 Object Carousel")
        }
        StreamType::METADATA_IN_DOWNLOAD_PROTOCOL => {
            Some("Metadata in ISO/IEC 13818-6 Synchronized Download Protocol")
        }
        StreamType::IPMP => Some("IPMP"),
        StreamType::H264 => Some("H.264"),
        StreamType::H265 => Some("H.265"),
        StreamType::USER_PRIVATE => Some("user private"),
        StreamType::AC3 => Some("AC-3"),
        StreamType::DTS => Some("DTS"),
        StreamType::TRUEHD => Some("TrueHD"),
        StreamType::DOLBY_DIGITAL_PLUS => Some("Dolby Digital Plus"),
        _ => None,
    }
}

struct CountedRead<T> {
    inner: T,
    count: u64,
}

impl<T> CountedRead<T> {
    #[inline]
    pub fn new(inner: T) -> CountedRead<T> {
        CountedRead { inner, count: 0 }
    }

    #[inline]
    pub fn count(&self) -> u64 {
        self.count
    }
}

impl<T: std::io::Read> std::io::Read for CountedRead<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let r = self.inner.read(buf);
        if let Ok(c) = r {
            self.count += c as u64;
        }
        r
    }
}

const HELP: &str = "\
パケットを数えるコマンド

USAGE:
  count [PATH]

FLAGS:
  -h, --help このヘルプを表示する

ARGS:
  <PATH>     パケットを数えるTSファイルのパス
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
    let mut f = CountedRead::new(f);

    let mut demuxer = isdb::demux::Demuxer::new(Filter::new());
    for packet in isdb::Packet::iter(&mut f) {
        let packet = packet?;

        demuxer.get_filter_mut().counter.input += 1;
        if packet.error_indicator() {
            demuxer.get_filter_mut().counter.transport_error += 1;
            continue;
        }
        if !packet.is_normal() {
            demuxer.get_filter_mut().counter.format_error += 1;
            continue;
        }

        let mut count = &mut demuxer.get_filter_mut().counter.counts[packet.pid()];
        count.input += 1;
        if packet.is_scrambled() {
            count.scrambled += 1;
        }

        demuxer.feed(&packet);
    }

    let Filter {
        services,
        emm_pids,
        counter,
        ..
    } = demuxer.into_filter();
    let continuity_error = counter
        .counts
        .iter()
        .map(|c| c.continuity_error)
        .sum::<u64>();
    let scrambled = counter.counts.iter().map(|c| c.scrambled).sum::<u64>();

    println!("Input Bytes     : {:9}", f.count());
    println!("Input Packets   : {:9}", counter.input);
    println!("Format Error    : {:9}", counter.format_error);
    println!("Transport Error : {:9}", counter.transport_error);
    println!("Dropped         : {:9}", continuity_error);
    println!("Scrambled       : {:9}", scrambled);
    println!();
    println!(" PID :     Input   Dropped Scrambled : Description");
    for (pid, count) in counter.counts.iter().enumerate() {
        if count.input != 0 {
            let pid = Pid::new(pid as u16);

            let mut pid_texts = Vec::new();
            if let Some(text) = pid_description(pid) {
                pid_texts.push(text.to_string());
            }
            if emm_pids.contains(&pid) {
                pid_texts.push("EMM".to_string());
            }
            for (service_id, svc) in &services {
                let service_id = format!("[{:04X}]", service_id);

                if svc.pmt_pids.contains(&pid) {
                    pid_texts.push(service_id);
                    pid_texts.push("PMT".to_string());
                } else if svc.pcr_pids.contains(&pid) {
                    pid_texts.push(service_id);
                    pid_texts.push("PCR".to_string());
                } else if svc.ecm_pids.contains(&pid) {
                    pid_texts.push(service_id);
                    pid_texts.push("ECM".to_string());
                } else if let Some(stream_type) = svc.stream_types.get(&pid) {
                    pid_texts.push(service_id);

                    match *stream_type {
                        isdb::psi::desc::StreamType::CAPTION => {
                            pid_texts.push("Caption".to_string())
                        }
                        isdb::psi::desc::StreamType::DATA_CARROUSEL => {
                            pid_texts.push("Data".to_string())
                        }
                        _ => {
                            if let Some(text) = stream_type_description(*stream_type) {
                                pid_texts.push(text.to_string());
                            }
                        }
                    };
                }
            }

            println!(
                "{:04X} : {:9} {:9} {:9} : {}",
                pid,
                count.input,
                count.continuity_error,
                count.scrambled,
                pid_texts.join(" "),
            );
        }
    }

    Ok(())
}
