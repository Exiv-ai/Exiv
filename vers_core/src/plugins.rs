// 🔌 Static Linker Configuration
// This file ensures that the plugin crates are linked into the binary so that `inventory` can discover them.
// To add a new plugin:
// 1. Add it to Cargo.toml
// 2. Add `use plugin_name;` here.

#[allow(unused_imports)]
use plugin_cerebras;
#[allow(unused_imports)]
use plugin_cursor;
#[allow(unused_imports)]
use plugin_deepseek;
#[allow(unused_imports)]
use plugin_ks2_2;
#[allow(unused_imports)]
use plugin_python_bridge;
