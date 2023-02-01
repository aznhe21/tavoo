use std::cell::RefCell;
use std::fs::File;
use std::io::BufReader;
use std::rc::Rc;

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

struct Filter(Rc<RefCell<Counter>>);

impl isdb::demux::Filter for Filter {
    fn on_pes_packet(&mut self, _: Pid, _: &[u8]) {}
    fn on_psi_section(&mut self, _: Pid, _: &isdb::psi::PsiSection) {}

    fn on_transport_error(&mut self) {
        let mut counter = self.0.borrow_mut();

        counter.input += 1;
        counter.transport_error += 1;
    }

    fn on_format_error(&mut self) {
        let mut counter = self.0.borrow_mut();

        counter.input += 1;
        counter.format_error += 1;
    }

    fn on_packet(&mut self, packet: &isdb::Packet) -> Option<isdb::demux::PacketType> {
        let mut counter = self.0.borrow_mut();

        counter.input += 1;

        let mut count = &mut counter.counts[packet.pid()];
        count.input += 1;
        if packet.is_scrambled() {
            count.scrambled += 1;
        }

        // PESの方が処理が軽い
        Some(isdb::demux::PacketType::Pes)
    }

    fn on_discontinued(&mut self, pid: Pid) {
        let mut counter = self.0.borrow_mut();

        counter.counts[pid].continuity_error += 1;
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

    let counter = Rc::new(RefCell::new(Counter {
        input: 0,
        format_error: 0,
        transport_error: 0,
        counts: isdb::pid::PidTable::from_fn(|_| Count::default()),
    }));

    let mut demuxer = isdb::demux::Demuxer::new(Filter(counter.clone()));
    for packet in isdb::Packet::iter(f) {
        demuxer.handle(&packet?);
    }

    let counter = counter.borrow();
    let continuity_error = counter
        .counts
        .iter()
        .map(|c| c.continuity_error)
        .sum::<u64>();
    let scrambled = counter.counts.iter().map(|c| c.scrambled).sum::<u64>();

    println!("Input Packets   : {:9}", counter.input);
    println!("Format Error    : {:9}", counter.format_error);
    println!("Transport Error : {:9}", counter.transport_error);
    println!("Dropped         : {:9}", continuity_error);
    println!("Scrambled       : {:9}", scrambled);
    println!();
    println!(" PID :     Input   Dropped Scrambled");
    for (pid, count) in counter.counts.iter().enumerate() {
        if count.input != 0 {
            println!(
                "{:04X} : {:9} {:9} {:9}",
                pid, count.input, count.continuity_error, count.scrambled,
            );
        }
    }

    Ok(())
}
