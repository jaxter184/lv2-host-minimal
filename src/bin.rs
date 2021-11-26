use lv2hm::*;

fn main() {
    audio_midi_instrument_test();
    audio_process_test();
    println!("I didn't crash!");
}

fn audio_midi_instrument_test(){
//    println!("{}", old_host.features_ptr as usize);
    let mut host = Lv2Host::new(1, 1, 44100);
    host.add_plugin("http://calf.sourceforge.net/plugins/Monosynth", "Organ".to_owned()).expect("Lv2hm: could not add plugin");
    host.set_value("Organ", "MIDI Channel", 0.0);

    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 44100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
//    println!("{}", old_host.features_ptr as usize);
//    println!("{}", host.features_ptr as usize);
    let mut writer = hound::WavWriter::create("midi-outp.wav", spec).unwrap();
    for i in 0..44100 {
        // alternate midi on and off messages, 5000 samples apart
        let mut midimsg = Vec::new();
        if (i % 10000) == 0 {
            midimsg.push((0, [0x90, 72, 96]))
        }
        else if (i % 5000) == 0 {
            midimsg.push((0, [0x80, 72, 96]))
        }
        let out = host.apply_multi(0, midimsg, [&[0.0], &[0.0]]).unwrap();
        let amplitude = i16::MAX as f32;
        writer.write_sample((out[0][0] * amplitude) as i16).unwrap();
        writer.write_sample((out[1][0] * amplitude) as i16).unwrap();
    }
}

fn audio_process_test(){
    let mut host = Lv2Host::new(1000, 1, 44100);
    host.add_plugin("http://calf.sourceforge.net/plugins/Reverb", "reverb".to_owned()).expect("Lv2hm: could not add plugin");
    host.add_plugin("http://calf.sourceforge.net/plugins/VintageDelay", "delay".to_owned()).expect("Lv2hm: could not add plugin");
    host.add_plugin("http://calf.sourceforge.net/plugins/Compressor", "compressor".to_owned()).expect("Lv2hm: could not add plugin");
    host.add_plugin("http://calf.sourceforge.net/plugins/Crusher", "crusher".to_owned()).expect("Lv2hm: could not add plugin");
    // host.remove_plugin("reverb");
    // host.remove_plugin("delay");

    let args: Vec<String> = std::env::args().collect();
    if args.len() <= 1 {
	    println!("expected an input file argument for audio test");
	    return
    }
    let file = &args[1];
    let mut reader = hound::WavReader::open(file).expect("Lv2hm: could not open audio file.");
    let specs = reader.spec();
    let mut writer = hound::WavWriter::create("audio-outp.wav", specs).unwrap();

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

        let (l, r) = host.apply(0, [0, 0, 0], (l, r));
        let (l, r) = host.apply(1, [0, 0, 0], (l, r));
        writer.write_sample((l * i16::MAX.abs() as f32) as i16)
            .expect("Error: could not write sample");
        writer.write_sample((r * i16::MAX.abs() as f32) as i16)
            .expect("Error: could not write sample");
    }
}
