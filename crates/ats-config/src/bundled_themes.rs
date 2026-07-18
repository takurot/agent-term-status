pub const BUNDLED_THEMES: &[(&str, &str)] = &[
    ("default", include_str!("../../../themes/default.yaml")),
    (
        "color-safe",
        include_str!("../../../themes/color-safe.yaml"),
    ),
    (
        "low-distraction",
        include_str!("../../../themes/low-distraction.yaml"),
    ),
    (
        "high-contrast",
        include_str!("../../../themes/high-contrast.yaml"),
    ),
    (
        "monochrome-symbols",
        include_str!("../../../themes/monochrome-symbols.yaml"),
    ),
];
