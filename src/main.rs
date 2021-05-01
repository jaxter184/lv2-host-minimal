mod lv2;
use crate::lv2::*;

fn main() {
    unsafe{
        let mut host = Lv2Host::new();
        host.add_plugin("http://calf.sourceforge.net/plugins/Compressor", "compressor".to_owned()).expect("TermDaw: could not add plugin");
        host.add_plugin("http://calf.sourceforge.net/plugins/Crusher", "crusher".to_owned()).expect("TermDaw: could not add plugin");
        let x = host.set_value("compressor", "Ratio", 2.0);
        println!("{}", x);
        // doesn't work if second+, port value locations change on vector resize?
        // host.add_plugin("http://calf.sourceforge.net/plugins/Reverb", "reverb".to_owned()).expect("TermDaw: could not add plugin");

        let args: Vec<String> = std::env::args().collect();
        let file = &args[1];
        let mut reader = hound::WavReader::open(file).expect("TermDaw: could not open audio file.");
        let specs = reader.spec();
        let mut writer = hound::WavWriter::create("outp.wav", specs).unwrap();

        let mut iter = reader.samples::<i16>();
        loop{
            let next = iter.next();
            if next.is_none() { break; }
            let s = next.unwrap();
            if s.is_err() { continue; }
            let l = s.unwrap() as f32 / i16::MAX.abs() as f32;
            let next = iter.next();
            if next.is_none() { break; }
            let s = next.unwrap();
            if s.is_err() { continue; }
            let r = s.unwrap() as f32 / i16::MAX.abs() as f32;

            let (l, r) = host.apply_plugin(0, (l,r));
            //let (l, r) = host.apply_plugin(1, (l,r));
            writer.write_sample((l * i16::MAX.abs() as f32) as i16)
                .expect("Error: could not write sample");
            writer.write_sample((r * i16::MAX.abs() as f32) as i16)
                .expect("Error: could not write sample");
        }
    }
    println!("I didn't crash!");
}

