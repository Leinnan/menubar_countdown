list:
    just --list

fix:
    -cargo fmt --all
    -cargo clippy --fix --allow-dirty --allow-staged -- -D warnings

# ---------------------------------------------------------------------------
# Icon generation
# Requires: rsvg-convert (brew install librsvg) and iconutil (macOS built-in)
# ---------------------------------------------------------------------------

svg     := "resources/icon.svg"
png     := "resources/icon.png"
iconset := "resources/icon.iconset"
icns    := "resources/icon.icns"

# Generate all icon formats (PNG + ICNS)
icons: png icns

# Render a 1024×1024 PNG from the SVG
png:
    rsvg-convert -w 1024 -h 1024 "{{svg}}" -o "{{png}}"
    @echo "Created {{png}}"

# Build a full .icns from the SVG (requires rsvg-convert + iconutil)
icns: _iconset
    iconutil -c icns "{{iconset}}" -o "{{icns}}"
    rm -rf "{{iconset}}"
    @echo "Created {{icns}}"

# Internal: populate the .iconset folder with all required sizes
_iconset:
    mkdir -p "{{iconset}}"
    rsvg-convert -w 16   -h 16   "{{svg}}" -o "{{iconset}}/icon_16x16.png"
    rsvg-convert -w 32   -h 32   "{{svg}}" -o "{{iconset}}/icon_16x16@2x.png"
    rsvg-convert -w 32   -h 32   "{{svg}}" -o "{{iconset}}/icon_32x32.png"
    rsvg-convert -w 64   -h 64   "{{svg}}" -o "{{iconset}}/icon_32x32@2x.png"
    rsvg-convert -w 128  -h 128  "{{svg}}" -o "{{iconset}}/icon_128x128.png"
    rsvg-convert -w 256  -h 256  "{{svg}}" -o "{{iconset}}/icon_128x128@2x.png"
    rsvg-convert -w 256  -h 256  "{{svg}}" -o "{{iconset}}/icon_256x256.png"
    rsvg-convert -w 512  -h 512  "{{svg}}" -o "{{iconset}}/icon_256x256@2x.png"
    rsvg-convert -w 512  -h 512  "{{svg}}" -o "{{iconset}}/icon_512x512.png"
    rsvg-convert -w 1024 -h 1024 "{{svg}}" -o "{{iconset}}/icon_512x512@2x.png"

# Clean generated icon files
icons-clean:
    rm -f "{{png}}" "{{icns}}"
    rm -rf "{{iconset}}"
