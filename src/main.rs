use lilv_sys::*;
// find plugins in /usr/lib/lv2
// URI list with lv2ls
// https://docs.rs/lilv-sys/0.2.1/lilv_sys/index.html
// https://github.com/wmedrano/Olivia/tree/main/lilv

// missing binding?
pub const LILV_URI_CONNECTION_OPTIONAL: &'static [u8; 49usize] = b"https://lv2plug.in/ns/lv2core#connectionOptional\0";

fn main() {
    unsafe{
        let world = lilv_world_new();
        let uri = lilv_new_uri(world, "http://calf.sourceforge.net/plugins/Compressor".as_ptr() as *const i8);
        lilv_world_load_all(world);
        let plugins = lilv_world_get_all_plugins(world);
        let plugin = lilv_plugins_get_by_uri(plugins, uri);
        lilv_node_free(uri);
        println!("{:?}", plugin); // bruh moment: if i dont print this plugin evaluates to 0x0 but if i do it's a value and it's fine.
        create_ports(world, plugin);
    }
    println!("Hello, world!");
}

enum PortType{ CONTROL, AUDIO }

struct Port{
    lilv_port: LilvPort,
    ptype: PortType,
    index: u32,
    value: f32,
    is_input: bool,
    optional: bool,
}

unsafe fn create_ports(world: *mut LilvWorld, plugin: *const LilvPluginImpl){
    println!("{:?}", plugin); // this checks if our weird ass pointer becomes 0x0 or not
    let n_ports = lilv_plugin_get_num_ports(plugin);
    let values = vec![0.0f32; n_ports as usize];
    lilv_plugin_get_port_ranges_float(plugin, std::ptr::null_mut(), std::ptr::null_mut(), values.as_ptr() as *mut f32);
    let lv2_input_port = lilv_new_uri(world, LILV_URI_INPUT_PORT.as_ptr() as *const i8);
    let lv2_output_port = lilv_new_uri(world, LILV_URI_OUTPUT_PORT.as_ptr() as *const i8);
    let lv2_audio_port = lilv_new_uri(world, LILV_URI_AUDIO_PORT.as_ptr() as *const i8);
    let lv2_control_port = lilv_new_uri(world, LILV_URI_CONTROL_PORT.as_ptr() as *const i8);
    let lv2_connection_optional = lilv_new_uri(world, LILV_URI_CONNECTION_OPTIONAL.as_ptr() as *const i8);
    println!("{}", n_ports);
    for i in 0..n_ports{
        println!("{}", i);
        let lport = lilv_plugin_get_port_by_index(plugin, i);
        let value = if values[i as usize].is_nan() { 0.0 } else { values[i as usize] };
        let optional = lilv_port_has_property(plugin, lport, lv2_connection_optional);
        let is_input = if lilv_port_is_a(plugin, lport, lv2_input_port) { true }
        else if lilv_port_is_a(plugin, lport, lv2_output_port) && !optional { println!("Port is neither input nor output."); false }
        else { false };

        let ptype = if lilv_port_is_a(plugin, lport, lv2_control_port) { PortType::CONTROL }
        else if lilv_port_is_a(plugin, lport, lv2_audio_port) { PortType::AUDIO }
        else if !optional { panic!("oof"); PortType::CONTROL }
        else { PortType::CONTROL };
    }
}
