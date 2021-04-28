use lilv_sys::*;
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
        let (ports, n_audio_in, n_audio_out) = create_ports(world, plugin);
    }
    println!("Hello, world!");
}

enum PortType{ Control, Audio, Other }

struct Port{
    lilv_port: *const LilvPort,
    ptype: PortType,
    index: u32,
    value: f32,
    is_input: bool,
    optional: bool,
}

// ([ports], n_audio_in, n_audio_out)
unsafe fn create_ports(world: *mut LilvWorld, plugin: *const LilvPluginImpl) -> (Vec<Port>, usize, usize){
    println!("{:?}", plugin); // this checks if our weird ass pointer becomes 0x0 or not
    if world == std::ptr::null_mut() { panic!("create_ports: world is null"); }
    if plugin == std::ptr::null_mut() { panic!("create_ports: plugin is null"); }

    let mut ports = Vec::new();
    let mut n_audio_in = 0;
    let mut n_audio_out = 0;

    let n_ports = lilv_plugin_get_num_ports(plugin);
    println!("Ports found: {}", n_ports);

    let values = vec![0.0f32; n_ports as usize];
    lilv_plugin_get_port_ranges_float(plugin, std::ptr::null_mut(), std::ptr::null_mut(), values.as_ptr() as *mut f32);

    let lv2_input_port = lilv_new_uri(world, LILV_URI_INPUT_PORT.as_ptr() as *const i8);
    let lv2_output_port = lilv_new_uri(world, LILV_URI_OUTPUT_PORT.as_ptr() as *const i8);
    let lv2_audio_port = lilv_new_uri(world, LILV_URI_AUDIO_PORT.as_ptr() as *const i8);
    let lv2_control_port = lilv_new_uri(world, LILV_URI_CONTROL_PORT.as_ptr() as *const i8);
    let lv2_connection_optional = lilv_new_uri(world, LILV_URI_CONNECTION_OPTIONAL.as_ptr() as *const i8);

    for i in 0..n_ports{
        let lport = lilv_plugin_get_port_by_index(plugin, i);
        let value = if values[i as usize].is_nan() { 0.0 } else { values[i as usize] };
        let optional = lilv_port_has_property(plugin, lport, lv2_connection_optional);

        let is_input = if lilv_port_is_a(plugin, lport, lv2_input_port) { true }
        else if !lilv_port_is_a(plugin, lport, lv2_output_port) && !optional { panic!("Port is neither input nor output."); }
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
        else if !optional { panic!("oof"); }
        else { PortType::Other };

        ports.push(Port{
            lilv_port: lport,
            ptype,
            index: i,
            value ,
            is_input,
            optional,
        });
    }

    lilv_node_free(lv2_input_port);
    lilv_node_free(lv2_output_port);
    lilv_node_free(lv2_audio_port);
    lilv_node_free(lv2_control_port);
    lilv_node_free(lv2_connection_optional);

    (ports, n_audio_in, n_audio_out)
}
