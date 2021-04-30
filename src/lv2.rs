use lilv_sys::*;
use std::ffi::{ CStr };
use std::ptr;
use std::ffi;
// find plugins in /usr/lib/lv2
// URI list with lv2ls
// https://docs.rs/lilv-sys/0.2.1/lilv_sys/index.html
// https://github.com/wmedrano/Olivia/tree/main/lilv
// UB fixed thanks to rust lang discord:
// https://discord.com/channels/273534239310479360/592856094527848449/837709621111947285

pub const LILV_URI_CONNECTION_OPTIONAL: &[u8; 48usize] = b"http://lv2plug.in/ns/lv2core#connectionOptional\0";

pub struct Lv2Host{
    world: *mut LilvWorld,
    lilv_plugins: *const LilvPlugins,
    plugins: Vec<Plugin>,
    in_buf: [f32; 2],
    out_buf: [f32; 2],
}

impl Lv2Host{
    pub unsafe fn new() -> Self{
        let world = lilv_world_new();
        lilv_world_load_all(world);
        let lilv_plugins = lilv_world_get_all_plugins(world);
        Lv2Host{
            world,
            lilv_plugins,
            plugins: Vec::new(),
            in_buf: [0.0; 2],
            out_buf: [0.0; 2],
        }
    }

    pub unsafe fn add_plugin(&mut self, uri: &str) -> Result<(), String>{
        let uri = lilv_new_uri(self.world, uri.as_ptr() as *const i8);
        let plugin = lilv_plugins_get_by_uri(self.lilv_plugins, uri);
        lilv_node_free(uri);

        let (mut ports, n_audio_in, n_audio_out) = create_ports(self.world, plugin);
        if n_audio_in != 2 || n_audio_out != 2 {
            return Err(format!("TermDaw: plugin audio input and output ports must be 2. \n\t Audio input ports: {}, Audio output ports: {}", n_audio_in, n_audio_out));
        }

        let instance = lilv_plugin_instantiate(plugin, 44100.0, ptr::null_mut());
        let (mut i, mut o) = (0, 0);
        for p in &mut ports{
            match p.ptype{
                PortType::Control => {
                    lilv_instance_connect_port(instance, p.index, &mut p.value as *mut f32 as *mut ffi::c_void);
                },
                PortType::Audio => {
                    if p.is_input {
                        lilv_instance_connect_port(instance, p.index, self.in_buf.as_ptr().offset(i) as *mut ffi::c_void);
                        i += 1;
                    } else {
                        lilv_instance_connect_port(instance, p.index, self.out_buf.as_ptr().offset(o) as *mut ffi::c_void);
                        o += 1;
                    }
                },
                PortType::Other => {
                    lilv_instance_connect_port(instance, p.index, ptr::null_mut());
                }
            }
        }

        lilv_instance_activate(instance);

        self.plugins.push(Plugin{
            lilv_plugin: plugin,
            instance,
        });

        Ok(())
    }

    pub unsafe fn apply_plugin(&mut self, index: usize, input_frame: (f32, f32)) -> (f32, f32){
        if index >= self.plugins.len() { return (0.0, 0.0); }
        let plugin = &mut self.plugins[index];
        self.in_buf[0] = input_frame.0;
        self.in_buf[1] = input_frame.1;
        lilv_instance_run(plugin.instance, 1);
        (self.out_buf[0], self.out_buf[1])
    }
}

impl Drop for Lv2Host{
    fn drop(&mut self){
        unsafe{
            for plugin in &self.plugins{
                lilv_instance_deactivate(plugin.instance);
                lilv_instance_free(plugin.instance);
            }
            lilv_world_free(self.world);
        }
    }
}

enum PortType{ Control, Audio, Other }

struct Plugin{
    lilv_plugin: *const LilvPlugin,
    instance: *mut LilvInstance,
}

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
    if world.is_null() { panic!("TermDaw: create_ports: world is null."); }
    if plugin.is_null() { panic!("TermDaw: create_ports: plugin is null."); }

    let mut ports = Vec::new();
    let mut n_audio_in = 0;
    let mut n_audio_out = 0;

    let n_ports = lilv_plugin_get_num_ports(plugin);
    println!("-------- Ports found: {}", n_ports);

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
