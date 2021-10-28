use lv2hm::*;

use urid::*;
use lv2_urid::*;
use std::pin::Pin;

fn main() {
    audio_midi_instrument_test();
    //audio_process_test();
    println!("I didn't crash!");
}

struct Host {
	host: Lv2Host,
	#[allow(dead_code)]
	host_map: Pin<Box<HostMap<HashURIDMapper>>>,
	features: Vec<*const lv2_raw::core::LV2Feature>,
	pub features_ptr: *const *const lv2_raw::core::LV2Feature,
	map_interface: lv2_sys::LV2_URID_Map,
}

impl Host {
	pub fn new() -> Self {
	    let mut host = Lv2Host::new(1000, 1, 44100);
	    let mut host_map: Pin<Box<HostMap<HashURIDMapper>>> = Box::pin(HashURIDMapper::new().into());
	    let mut map_interface = host_map.as_mut().make_map_interface();
	    let map = LV2Map::new(&map_interface);
	    host.set_maps(&map);
	    // let map_ptr = map_interface.handle;
	    let mapf = lv2_raw::core::LV2Feature {
	        uri: LV2_URID_MAP.as_ptr() as *const i8,
	        data: &mut map_interface as *mut lv2_sys::LV2_URID_Map as *mut std::ffi::c_void,
	    };
	    let mapfp = &mapf as *const lv2_raw::core::LV2Feature;
	    let features = vec![mapfp, std::ptr::null::<lv2_raw::core::LV2Feature>()];
	    let features_ptr = features.as_ptr() as *const *const lv2_raw::core::LV2Feature;
	    host.printmap();
	    Self {
		    host,
		    host_map,
		    features,
		    features_ptr,
		    map_interface,
	    }
	}
}

// doesn't work yet
fn audio_midi_instrument_test(){
	let mut old_host = Host::new();
	println!("{}", old_host.features_ptr as usize);
	std::thread::sleep(std::time::Duration::from_secs(1));
	let mut host = Host::new();
	println!("{}", host.features_ptr as usize);
//    host.add_plugin("http://calf.sourceforge.net/plugins/Monosynth", "Organ".to_owned(), features_ptr).expect("Lv2hm: could not add plugin");
    host.host.add_plugin("https://github.com/RustAudio/rust-lv2/tree/master/docs/amp", "Organ".to_owned(), host.features_ptr).expect("Lv2hm: could not add plugin");
    host.host.set_value("Organ", "MIDI Channel", 0.0);

    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 44100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
	println!("{}", old_host.features_ptr as usize);
	println!("{}", host.features_ptr as usize);
    let mut writer = hound::WavWriter::create("outp.wav", spec).unwrap();
    for i in 0..44100 {
	    // alternate midi on and off messages, 5000 samples apart
        let bytes = if (i % 10000) == 0 {
            Some([0x90, 74, 96])
        }
        else if (i % 5000) == 0 {
            Some([0x80, 74, 96])
        }
        else {
	        None
        };
    //    let bytes = Some([0x90, 74, 96]);
        let (l, r) = host.host.apply_midi(0, bytes, (0.0, 0.0));
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
