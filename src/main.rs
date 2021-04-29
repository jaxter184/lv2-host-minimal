use lilv_sys::*;
use std::ffi::{ CStr };
use std::ptr;
use std::ffi;
// find plugins in /usr/lib/lv2
// URI list with lv2ls
// https://docs.rs/lilv-sys/0.2.1/lilv_sys/index.html
// https://github.com/wmedrano/Olivia/tree/main/lilv

// missing binding?
pub const LILV_URI_CONNECTION_OPTIONAL: &[u8; 48usize] = b"http://lv2plug.in/ns/lv2core#connectionOptional\0";

fn main() {
    unsafe{
        let world = lilv_world_new();
        let uri = lilv_new_uri(world, "http://calf.sourceforge.net/plugins/Compressor".as_ptr() as *const i8);
        lilv_world_load_all(world);
        let plugins = lilv_world_get_all_plugins(world);
        let plugin = lilv_plugins_get_by_uri(plugins, uri);
        lilv_node_free(uri);
        println!("{:?}", plugin); // bruh moment: if i dont print this plugin evaluates to 0x0 but if i do it's a value and it's fine.
        let (mut ports, n_audio_in, n_audio_out) = create_ports(world, plugin);
        if n_audio_in != 2 || n_audio_out != 2 {
            panic!("TermDaw: plugin audio input and output ports must be 2. \n\t Audio input ports: {}, Audio output ports: {}", n_audio_in, n_audio_out);
        }

        let mut in_buf = [0.0f32; 2];
        let out_buf = [0.0f32; 2];
        let instance = lilv_plugin_instantiate(plugin, 44100.0, ptr::null_mut());
        let (mut i, mut o) = (0, 0);
        for p in &mut ports{
            match p.ptype{
                PortType::Control => {
                    lilv_instance_connect_port(instance, p.index, &mut p.value as *mut f32 as *mut ffi::c_void);
                },
                PortType::Audio => {
                    if p.is_input {
                        lilv_instance_connect_port(instance, p.index, in_buf.as_ptr().offset(i) as *mut ffi::c_void);
                        i += 1;
                    } else {
                        lilv_instance_connect_port(instance, p.index, out_buf.as_ptr().offset(o) as *mut ffi::c_void);
                        o += 1;
                    }
                },
                PortType::Other => {
                    lilv_instance_connect_port(instance, p.index, ptr::null_mut());
                }
            }
        }

        let args: Vec<String> = std::env::args().collect();
        let file = &args[1];
        let mut reader = hound::WavReader::open(file).expect("TermDaw: could not open audio file.");
        let specs = reader.spec();
        let mut writer = hound::WavWriter::create("outp.wav", specs).unwrap();

        lilv_instance_activate(instance);

        let mut x = 0;
        for s in reader.samples::<i16>(){
            if s.is_err() { continue; }
            let s = s.unwrap();
            in_buf[x] = s as f32 / i16::MAX.abs() as f32;
            x += 1;
            if x == 2 {
                x = 0;
                lilv_instance_run(instance, 1);
                writer.write_sample((out_buf[0] * i16::MAX.abs() as f32) as i16)
                    .expect("Error: could not write sample");
                writer.write_sample((out_buf[1] * i16::MAX.abs() as f32) as i16)
                    .expect("Error: could not write sample");
            }
        }

        lilv_instance_deactivate(instance);
        lilv_instance_free(instance);
        lilv_world_free(world);
    }
    println!("I didn't crash!");
}

enum PortType{ Control, Audio, Other }

struct Port{
    lilv_port: *const LilvPort,
    index: u32,
    optional: bool,
    is_input: bool,
    ptype: PortType,
    value: f32,
    def: f32,
    min: f32,
    max: f32,
    name: String,
}

// ([ports], n_audio_in, n_audio_out)
unsafe fn create_ports(world: *mut LilvWorld, plugin: *const LilvPluginImpl) -> (Vec<Port>, usize, usize){
    println!("{:?}", plugin); // this checks if our weird ass pointer becomes 0x0 or not
    if world.is_null() { panic!("TermDaw: create_ports: world is null."); }
    if plugin.is_null() { panic!("TermDaw: create_ports: plugin is null."); }

    let mut ports = Vec::new();
    let mut n_audio_in = 0;
    let mut n_audio_out = 0;

    let n_ports = lilv_plugin_get_num_ports(plugin);
    println!("Ports found: {}", n_ports);

    let mins = vec![0.0f32; n_ports as usize];
    let maxs = vec![0.0f32; n_ports as usize];
    let defs = vec![0.0f32; n_ports as usize];
    lilv_plugin_get_port_ranges_float(plugin, mins.as_ptr() as *mut f32, maxs.as_ptr() as *mut f32, defs.as_ptr() as *mut f32);

    let lv2_input_port = lilv_new_uri(world, LILV_URI_INPUT_PORT.as_ptr() as *const i8);
    let lv2_output_port = lilv_new_uri(world, LILV_URI_OUTPUT_PORT.as_ptr() as *const i8);
    let lv2_audio_port = lilv_new_uri(world, LILV_URI_AUDIO_PORT.as_ptr() as *const i8);
    let lv2_control_port = lilv_new_uri(world, LILV_URI_CONTROL_PORT.as_ptr() as *const i8);
    let lv2_connection_optional = lilv_new_uri(world, LILV_URI_CONNECTION_OPTIONAL.as_ptr() as *const i8);

    for i in 0..n_ports{
        let lport = lilv_plugin_get_port_by_index(plugin, i);
        let def = defs[i as usize];
        let min = mins[i as usize];
        let max = maxs[i as usize];
        let value = if def.is_nan() { 0.0 } else { def };
        let lilv_name = lilv_port_get_name(plugin, lport);
        let lilv_str = lilv_node_as_string(lilv_name);
        let c_str = CStr::from_ptr(lilv_str as *const i8);
        let name = c_str.to_str().expect("TermDaw: could not build port name string.").to_owned();
        println!("{}: {} in [{}, {}]", name, value, mins[i as usize], maxs[i as usize]);
        let optional = lilv_port_has_property(plugin, lport, lv2_connection_optional);

        let is_input = if lilv_port_is_a(plugin, lport, lv2_input_port) { true }
        else if !lilv_port_is_a(plugin, lport, lv2_output_port) && !optional { panic!("TermDaw: Port is neither input nor output."); }
        else { false };

        let ptype = if lilv_port_is_a(plugin, lport, lv2_control_port) { PortType::Control }
        else if lilv_port_is_a(plugin, lport, lv2_audio_port) {
            if is_input{
                n_audio_in += 1;
            } else {
                n_audio_out += 1;
            }
            PortType::Audio
        }
        else if !optional { panic!("TermDaw: port is neither a control, audio or optional port."); }
        else { PortType::Other };

        ports.push(Port{
            lilv_port: lport,
            index: i,
            ptype, is_input, optional, value, def, min, max, name,
        });
    }

    lilv_node_free(lv2_input_port);
    lilv_node_free(lv2_output_port);
    lilv_node_free(lv2_audio_port);
    lilv_node_free(lv2_control_port);
    lilv_node_free(lv2_connection_optional);

    (ports, n_audio_in, n_audio_out)
}
