use lilv_sys::*;
// find plugins in /usr/lib/lv2

fn main() {
    unsafe{
        let world = lilv_world_new();
        let uri = lilv_new_uri(world, "uri:/usr/lib/lv2/calf.lv2/Compressor.ttl".as_ptr() as *const i8);
        lilv_world_load_all(world);
        let plugins = lilv_world_get_all_plugins(world);
        let plugin = lilv_plugins_get_by_uri(plugins, uri);
        lilv_node_free(uri);
    }
    println!("Hello, world!");
}
