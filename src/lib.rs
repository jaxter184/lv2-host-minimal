use lilv_sys::*;

use std::ffi::{ CStr };
use std::ptr;
use std::ffi;
use std::collections::HashMap;

// find plugins in /usr/lib/lv2
// URI list with lv2ls
// https://docs.rs/lilv-sys/0.2.1/lilv_sys/index.html
// https://github.com/wmedrano/Olivia/tree/main/lilv
// UB fixed thanks to rust lang discord:
// https://discord.com/channels/273534239310479360/592856094527848449/837709621111947285

const LILV_URI_CONNECTION_OPTIONAL: &[u8; 48usize] = b"http://lv2plug.in/ns/lv2core#connectionOptional\0";
pub const LV2_URID_MAP: &[u8; 34usize] = b"http://lv2plug.in/ns/ext/urid#map\0";

pub struct Lv2Host{
    world: *mut LilvWorld,
    lilv_plugins: *const LilvPlugins,
    plugin_cap: usize,
    plugins: Vec<Plugin>,
    uri_home: Vec<String>,
    plugin_names: HashMap<String, usize>,
    dead_list: Vec<usize>,
    #[allow(dead_code)]
    buffer_len: usize,
    sr: f64,
    in_buf: Vec<f32>,
    out_buf: Vec<f32>,
    atom_buf: [u8; 1024],
    atom_seq_urid: Option<[u8; 4]>,
    midi_type_urid: Option<[u8; 4]>,
}

#[derive(Debug)]
pub enum AddPluginError{
    CapacityReached,
    MoreThanTwoInOrOutAudioPorts(usize, usize),
    MoreThanOneAtomPort(usize),
    WorldIsNull,
    PluginIsNull,
    PortNeitherInputOrOutput,
    PortNeitherControlOrAudioOrOptional,
}

impl Lv2Host{
    pub fn new(plugin_cap: usize, buffer_len: usize, sample_rate: usize) -> Self{
        let (world, lilv_plugins) = unsafe{
            let world = lilv_world_new();
            lilv_world_load_all(world);
            let lilv_plugins = lilv_world_get_all_plugins(world);
            (world, lilv_plugins)
        };
        Lv2Host{
            world,
            lilv_plugins,
            plugin_cap,
            plugins: Vec::with_capacity(plugin_cap), // don't let it resize: keep memory where it is
            uri_home: Vec::new(), // need to keep strings alive otherwise lilv memory goes boom
            plugin_names: HashMap::new(),
            dead_list: Vec::new(),
            buffer_len,
            sr: sample_rate as f64,
            in_buf: vec![0.0; buffer_len * 2], // also don't let it resize
            out_buf: vec![0.0; buffer_len * 2], // also don't let it resize
            atom_buf: [0; 1024],
            atom_seq_urid: None,
            midi_type_urid: None,
        }
    }

    pub fn printmap(&self) {
	    println!("pmm: {:?}", self.midi_type_urid);
	    println!("pma: {:?}", self.atom_seq_urid);
    }

    pub fn set_maps(&mut self, map: &lv2_urid::LV2Map) {
	    use urid::Map;
	    self.midi_type_urid = map.map_str("http://lv2plug.in/ns/ext/midi#MidiEvent").map(|inner| inner.get().to_le_bytes());
	    self.atom_seq_urid = map.map_str("http://lv2plug.in/ns/ext/atom#Sequence").map(|inner| inner.get().to_le_bytes());
	    self.midi_type_urid.unwrap();
	    self.atom_seq_urid.unwrap();
    }

    pub fn get_index(&self, name: &str) -> Option<usize>{
        self.plugin_names.get(name).copied()
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn add_plugin(&mut self, uri: &str, name: String, features_ptr: *const *const lv2_raw::core::LV2Feature) -> Result<(), AddPluginError>{
        let replace_index = self.dead_list.pop();
        if self.plugins.len() == self.plugin_cap && replace_index == None{
            return Err(AddPluginError::CapacityReached);
        }
        let mut uri_src = uri.to_owned();
        uri_src.push('\0'); // make sure it's null terminated
        self.uri_home.push(uri_src);
        let plugin = unsafe{
            let uri = lilv_new_uri(self.world, (&self.uri_home[self.uri_home.len() - 1]).as_ptr() as *const i8);
            let plugin = lilv_plugins_get_by_uri(self.lilv_plugins, uri);
            lilv_node_free(uri);
            plugin
        };

        let (mut ports, port_names, n_audio_in, n_audio_out, n_atom_in) = unsafe {
            match create_ports(self.world, plugin) {
                Ok(x) => x,
                Err(e) => { return Err(e); },
            }
        };

        if n_audio_in > 2 || n_audio_out > 2 {
            return Err(AddPluginError::MoreThanTwoInOrOutAudioPorts(n_audio_in, n_audio_out));
        }

        if n_atom_in > 1 {
            return Err(AddPluginError::MoreThanOneAtomPort(n_atom_in));
        }

        let instance = unsafe{
            //let map_feature_uri = lilv_new_uri(self.world, LV2_URID_MAP.as_ptr() as *const i8);
            //let map_feature_bool = lilv_plugin_has_feature(plugin, map_feature_uri);
            //println!("supports map: {}", map_feature_bool); // prints true, supports map
            let instance = lilv_plugin_instantiate(plugin, self.sr, features_ptr);
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
                    PortType::Atom => {
                        lilv_instance_connect_port(instance, p.index, self.atom_buf.as_ptr() as *mut ffi::c_void);
                    }
                    PortType::Other => {
                        lilv_instance_connect_port(instance, p.index, ptr::null_mut());
                    }
                }
            }

            lilv_instance_activate(instance);
            instance
        };

        let p = Plugin{
            instance,
            port_names,
            ports,
        };

        if let Some(i) = replace_index{
            self.plugins[i] = p;
            self.plugin_names.insert(name, i);
        } else {
            self.plugins.push(p);
            self.plugin_names.insert(name, self.plugins.len() - 1);
        }

        Ok(())
    }

    pub fn remove_plugin(&mut self, name: &str) -> bool{
        if let Some(index) = self.plugin_names.get(name){
            unsafe{
                lilv_instance_deactivate(self.plugins[*index].instance);
                // can't do this?
                // lilv_instance_free(self.plugins[*index].instance);
            }
            self.dead_list.push(*index);
            self.plugin_names.remove(name);
            true
        } else {
            false
        }
    }

    fn set_port_value(plug: &mut Plugin, port: &str, value: Option<f32>) -> bool{
            let port_index = plug.port_names.get(port);
            if port_index.is_none() { return false; }
            let port_index = port_index.unwrap();
            let min = plug.ports[*port_index].min;
            let max = plug.ports[*port_index].max;
            let value = if let Some(v) = value { v }
            else { plug.ports[*port_index].def };
            plug.ports[*port_index].value = value.max(min).min(max);
            true
    }

    pub fn set_value(&mut self, plugin: &str, port: &str, value: f32) -> bool{
        if let Some(index) = self.plugin_names.get(plugin){
            let plug = &mut self.plugins[*index];
            Self::set_port_value(plug, port, Some(value))
        } else {
            false
        }
    }

    pub fn reset_value(&mut self, plugin: &str, port: &str) -> bool{
        if let Some(index) = self.plugin_names.get(plugin){
            let plug = &mut self.plugins[*index];
            Self::set_port_value(plug, port, None)
        } else {
            false
        }
    }

    pub fn get_plugin_sheet(&self, index: usize) -> PluginSheet{
        let plug = &self.plugins[index];
        let mut ains = 0;
        let mut aouts = 0;
        let mut controls = Vec::new();
        for port in &plug.ports{
            if port.ptype == PortType::Audio{
                if port.is_input{
                    ains += 1;
                } else {
                    aouts += 1;
                }
            } else if port.ptype == PortType::Control && port.is_input{
                controls.push((port.name.clone(), port.def, port.min, port.max));
            }
        }
        PluginSheet{
            audio_ins: ains,
            audio_outs: aouts,
            controls,
        }
    }

    pub fn apply_plugin(&mut self, index: usize, input_frame: (f32, f32)) -> (f32, f32){
        if index >= self.plugins.len() { return (0.0, 0.0); }
        let plugin = &mut self.plugins[index];
        self.in_buf[0] = input_frame.0;
        self.in_buf[1] = input_frame.1;
        unsafe {
            lilv_instance_run(plugin.instance, 1);
        }
        (self.out_buf[0], self.out_buf[1])
    }

    // TODO: fix
    // pub fn _apply_plugin_n_frames(&mut self, index: usize, input: &[f32]) -> Option<&[f32]>{
    //     let frames = input.len() / 2;
    //     if frames > self.buffer_len { return None; }
    //     if index >= self.plugins.len() { return None; }
    //     for (i, v) in input.iter().enumerate(){
    //         self.in_buf[i] = *v;
    //     }
    //     let plugin = &mut self.plugins[index];
    //     unsafe{
    //         lilv_instance_run(plugin.instance, frames as u32);
    //     }
    //     Some(&self.out_buf)
    // }

    pub fn apply_instrument(&mut self, index: usize, input: &[u8]) -> (f32, f32){
        if index >= self.plugins.len() { return (0.0, 0.0); }
        for (i, v) in input.iter().enumerate() {
            self.atom_buf[i] = *v;
        }
        let plugin = &mut self.plugins[index];
        unsafe {
            lilv_instance_run(plugin.instance, 1);
        }
        (self.out_buf[0], self.out_buf[1])
    }

    pub fn apply_midi(&mut self, index: usize, input: Option<[u8; 3]>, input_frame: (f32, f32)) -> (f32, f32) {
	    let midi_urid_bytes = self.midi_type_urid.unwrap();
	    let atom_urid_bytes = self.atom_seq_urid.unwrap();
	    if let Some(inner) = input {
	    	let buffer = test_midi_atom(midi_urid_bytes, atom_urid_bytes, inner);
		    println!("lv2hm midi: {:?}", buffer);
	        for (i, v) in buffer.iter().enumerate() {
	            self.atom_buf[i] = *v;
	        }
	    }
	    else {
		    let buffer = [8,0,0,0, atom_urid_bytes[0], atom_urid_bytes[1], atom_urid_bytes[2], atom_urid_bytes[3], 0,0,0,0,0,0,0,0,];
	        for (i, v) in buffer.iter().enumerate() {
	            self.atom_buf[i] = *v;
	        }
	    };
        if index >= self.plugins.len() { panic!() }
        self.in_buf[0] = input_frame.0;
        self.in_buf[1] = input_frame.1;
        let plugin = &mut self.plugins[index];
        unsafe {
            lilv_instance_run(plugin.instance, 1);
        }
        (self.out_buf[0], self.out_buf[1])
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


impl Drop for Lv2Host{
    fn drop(&mut self){
        unsafe{
            for (i, plugin) in self.plugins.iter().enumerate(){
                if self.dead_list.contains(&i){
                    continue;
                }
                lilv_instance_deactivate(plugin.instance);
                lilv_instance_free(plugin.instance);
            }
            lilv_world_free(self.world);
        }
    }
}

struct Plugin{
    instance: *mut LilvInstance,
    port_names: HashMap<String, usize>,
    ports: Vec<Port>,
}

#[derive(Debug)]
pub struct PluginSheet{
    pub audio_ins: usize,
    pub audio_outs: usize,
    pub controls: Vec<(String, f32, f32, f32)>,
}

#[derive(PartialEq,Eq,Debug)]
enum PortType{ Control, Audio, Atom, Other }

#[derive(Debug)]
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

type CreatePortsRes = (Vec<Port>, HashMap<String, usize>, usize, usize, usize);

// ([ports], [(port_name, index)], n_audio_in, n_audio_out, n_atom_in)
unsafe fn create_ports(world: *mut LilvWorld, plugin: *const LilvPluginImpl) -> Result<CreatePortsRes, AddPluginError>{
    if world.is_null() { return Err(AddPluginError::WorldIsNull); }
    if plugin.is_null() { return Err(AddPluginError::PluginIsNull); }

    let mut ports = Vec::new();
    let mut names = HashMap::new();
    let mut n_audio_in = 0;
    let mut n_audio_out = 0;
    let mut n_atom_in = 0;

    let n_ports = lilv_plugin_get_num_ports(plugin);

    let mins = vec![0.0f32; n_ports as usize];
    let maxs = vec![0.0f32; n_ports as usize];
    let defs = vec![0.0f32; n_ports as usize];
    lilv_plugin_get_port_ranges_float(plugin, mins.as_ptr() as *mut f32, maxs.as_ptr() as *mut f32, defs.as_ptr() as *mut f32);

    let lv2_input_port = lilv_new_uri(world, LILV_URI_INPUT_PORT.as_ptr() as *const i8);
    let lv2_output_port = lilv_new_uri(world, LILV_URI_OUTPUT_PORT.as_ptr() as *const i8);
    let lv2_audio_port = lilv_new_uri(world, LILV_URI_AUDIO_PORT.as_ptr() as *const i8);
    let lv2_control_port = lilv_new_uri(world, LILV_URI_CONTROL_PORT.as_ptr() as *const i8);
    let lv2_atom_port = lilv_new_uri(world, LILV_URI_ATOM_PORT.as_ptr() as *const i8);
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
        let name = c_str.to_str().expect("Lv2hm: could not build port name string.").to_owned();

        names.insert(name.clone(), i as usize);

        let optional = lilv_port_has_property(plugin, lport, lv2_connection_optional);

        let is_input = if lilv_port_is_a(plugin, lport, lv2_input_port) { true }
        else if !lilv_port_is_a(plugin, lport, lv2_output_port) && !optional { return Err(AddPluginError::PortNeitherInputOrOutput); }
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
        else if lilv_port_is_a(plugin, lport, lv2_atom_port) && is_input{
            n_atom_in += 1;
            PortType::Atom
        }
        else if !optional { return Err(AddPluginError::PortNeitherControlOrAudioOrOptional); }
        else { PortType::Other };

        ports.push(Port{
            lilv_port: lport,
            index: i,
            ptype, is_input, optional, value, def, min, max, name,
        });
    }

    lilv_node_free(lv2_connection_optional);
    lilv_node_free(lv2_atom_port);
    lilv_node_free(lv2_control_port);
    lilv_node_free(lv2_audio_port);
    lilv_node_free(lv2_output_port);
    lilv_node_free(lv2_input_port);

    Ok((ports, names, n_audio_in, n_audio_out, n_atom_in))
}
