# lv2-host-minimal
Simple library to host lv2 plugins.
Is not meant to support any kind of GUI.

- [x] Host fx plugins (audio in, audio out)
- [x] Set parameters
- [x] Host midi instruments (midi in, audio out)

## Example

```rust
let mut host = Lv2Host::new(1, 1, 44100);
host.add_plugin("http://calf.sourceforge.net/plugins/Monosynth", "Organ".to_owned()).expect("Lv2hm: could not add plugin");
host.set_value("Organ", "MIDI Channel", 0.0);

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
    // do something with your audio
}
```
