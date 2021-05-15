# lv2-host-minimal
Simple library to host lv2 plugins.
Is not meant to support any kind of GUI.

- [x] Host fx plugins (audio in, audio out)
- [x] Set parameters
- [ ] Host midi instrumenst (midi in, audio out)

I could not get midi going.
You can see me trying in lib.rs

```rust
// set up a host with max 1000 plugins and a buffer length of 1
let mut host = Lv2Host::new(1000, 1);
// add some plugins
host.add_plugin("http://calf.sourceforge.net/plugins/Compressor", "compressor".to_owned(), std::ptr::null_mut()).expect("Lv2hm: could not add plugin");
host.add_plugin("http://calf.sourceforge.net/plugins/Crusher", "crusher".to_owned(), std::ptr::null_mut()).expect("Lv2hm: could not add plugin");
// set a parameter
host.set_value("compressor", "Knee", 4.0);
// print all ports of first plugin
println!("{:?}", host.get_plugin_sheet(0));
// some loop where you get your data
loop{
    // you have some audio frame
    let (l, r) = some_way_to_get_your_audio_frame();
    // apply the plugins to the frame
    let (l, r) = host.apply_plugin(0, (l,r));
    let (l, r) = host.apply_plugin(1, (l,r));
}
```
