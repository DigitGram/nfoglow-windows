// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 DigitGram
//! For Windows: the icon and the version information inside nfoglow.exe (shown by Explorer and the taskbar).
fn main() {
    println!("cargo:rerun-if-changed=assets/nfoglow.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/nfoglow.ico")
            .set("ProductName", "NfoGlow")
            .set("FileDescription", "NfoGlow")
            .set("CompanyName", "DigitGram")
            .set("LegalCopyright", "© 2026 DigitGram, GPL-3.0-or-later");
        res.compile().expect("the Windows resources (icon, version) could not be built");
    }
}
