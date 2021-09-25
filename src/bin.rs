use lv2hm::*;

use urid::*;
use lv2_urid::*;
use std::pin::Pin;

fn main() {
    audio_midi_instrument_test();
    //audio_process_test();
    println!("I didn't crash!");
}

// doesn't work yet
fn audio_midi_instrument_test(){
    let mut host = Lv2Host::new(1000, 1, 44100);
    let mut host_map: Pin<Box<HostMap<HashURIDMapper>>> = Box::pin(HashURIDMapper::new().into());
    let mut map_interface = host_map.as_mut().make_map_interface();
    let map = LV2Map::new(&map_interface);
    let midi_type_urid = map.map_str("http://lv2plug.in/ns/ext/midi#MidiEvent").unwrap();
    let atom_seq_urid = map.map_str("http://lv2plug.in/ns/ext/atom#Sequence").unwrap();
    let bytes = midi_type_urid.get().to_le_bytes();
    println!("{}, {:?}", midi_type_urid.get(), bytes);
    // let map_ptr = map_interface.handle;
    let mapf = lv2_raw::core::LV2Feature {
        uri: LV2_URID_MAP.as_ptr() as *const i8,
        data: &mut map_interface as *mut lv2_sys::LV2_URID_Map as *mut std::ffi::c_void,
    };
    let mapfp = &mapf as *const lv2_raw::core::LV2Feature;
    let features = vec![mapfp, std::ptr::null::<lv2_raw::core::LV2Feature>()];
    let features_ptr = features.as_ptr() as *const *const lv2_raw::core::LV2Feature;
    host.add_plugin("http://calf.sourceforge.net/plugins/Monosynth", "Organ".to_owned(), features_ptr).expect("Lv2hm: could not add plugin");
    host.set_value("Organ", "MIDI Channel", 0.0);

    // set up atom bytestreams
    let asbytes = atom_seq_urid.get().to_le_bytes();
    let midi_on = test_midi_atom(bytes, asbytes, [0x90, 50, 100]);
    let midi_off = test_midi_atom(bytes, asbytes, [0x80, 50, 100]);
    let reset = [8,0,0,0, asbytes[0], asbytes[1], asbytes[2], asbytes[3], 0,0,0,0,0,0,0,0,];

    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 44100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create("outp.wav", spec).unwrap();
    for i in 0..44100 {
	    // alternate midi on and off messages, 5000 samples apart
        let bytes = if (i % 10000) == 0 {
            &midi_on[..]
        }
        else if (i % 5000) == 0 {
            &midi_off[..]
        }
        else {
            &reset
        };
        let (l, r) = host.apply_instrument(0, bytes);
        let amplitude = i16::MAX as f32;
        writer.write_sample((l * amplitude) as i16).unwrap();
        writer.write_sample((r * amplitude) as i16).unwrap();
    }
}

fn audio_process_test(){
    let mut host = Lv2Host::new(1000, 1, 44100);
    host.add_plugin("http://calf.sourceforge.net/plugins/Reverb", "reverb".to_owned(), std::ptr::null_mut()).expect("Lv2hm: could not add plugin");
    host.add_plugin("http://calf.sourceforge.net/plugins/VintageDelay", "delay".to_owned(), std::ptr::null_mut()).expect("Lv2hm: could not add plugin");
    host.add_plugin("http://calf.sourceforge.net/plugins/Compressor", "compressor".to_owned(), std::ptr::null_mut()).expect("Lv2hm: could not add plugin");
    host.add_plugin("http://calf.sourceforge.net/plugins/Crusher", "crusher".to_owned(), std::ptr::null_mut()).expect("Lv2hm: could not add plugin");
    // host.remove_plugin("reverb");
    // host.remove_plugin("delay");
    println!("{:?}", host.get_plugin_sheet(0));

    let args: Vec<String> = std::env::args().collect();
    let file = &args[1];
    let mut reader = hound::WavReader::open(file).expect("Lv2hm: could not open audio file.");
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
        let (l, r) = host.apply_plugin(1, (l,r));
        writer.write_sample((l * i16::MAX.abs() as f32) as i16)
            .expect("Error: could not write sample");
        writer.write_sample((r * i16::MAX.abs() as f32) as i16)
            .expect("Error: could not write sample");
    }
}

fn test_midi_atom(typebytes: [u8; 4], seqbytes: [u8; 4], midibytes: [u8; 3]) -> [u8;38]{
    [
        // size
        32, 0, 0, 0,
        // type
        seqbytes[0], seqbytes[1], seqbytes[2], seqbytes[3],
        // timestamp
        0,0,0,0,0,0,0,0, // frame
        0,0,0,0,0,0,0,0, // subframe
        // size
        3, 0, 0, 0,
        // type
        typebytes[0], typebytes[1], typebytes[2], typebytes[3],
        // midi
        midibytes[0],
        midibytes[1],
        midibytes[2],
        // 32 bit pad (not sure if this is necessary)
        0,0,0,
    ]
}
