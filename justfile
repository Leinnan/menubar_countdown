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

# ---------------------------------------------------------------------------
# App bundle
# ---------------------------------------------------------------------------

app     := "target/release/menubar-countdown.app"
bin     := "target/release/menubar-countdown"
plist   := app + "/Contents/Info.plist"

# Build release binary, generate icons, and assemble the .app bundle
bundle: icns
    cargo build --release
    rm -rf "{{app}}"
    mkdir -p "{{app}}/Contents/MacOS"
    mkdir -p "{{app}}/Contents/Resources"
    cp "{{bin}}"  "{{app}}/Contents/MacOS/menubar-countdown"
    cp "{{icns}}" "{{app}}/Contents/Resources/icon.icns"
    just _plist
    @echo "Created {{app}}"

# Write Info.plist into the bundle
_plist:
    #!/usr/bin/env bash
    cat > "{{plist}}" <<'EOF'
    <?xml version="1.0" encoding="UTF-8"?>
    <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
    <plist version="1.0">
    <dict>
        <key>CFBundleExecutable</key>
        <string>menubar-countdown</string>
        <key>CFBundleIdentifier</key>
        <string>com.mevlyshkin.menubar-countdown</string>
        <key>CFBundleName</key>
        <string>Menubar Countdown</string>
        <key>CFBundleDisplayName</key>
        <string>Menubar Countdown</string>
        <key>CFBundleVersion</key>
        <string>0.1.0</string>
        <key>CFBundleShortVersionString</key>
        <string>0.1.0</string>
        <key>CFBundlePackageType</key>
        <string>APPL</string>
        <key>CFBundleSignature</key>
        <string>????</string>
        <key>CFBundleIconFile</key>
        <string>icon</string>
        <key>LSMinimumSystemVersion</key>
        <string>11.0</string>
        <key>LSUIElement</key>
        <true/>
        <key>NSHighResolutionCapable</key>
        <true/>
        <key>NSPrincipalClass</key>
        <string>NSApplication</string>
    </dict>
    </plist>
    EOF

# Remove the assembled .app bundle
bundle-clean:
    rm -rf "{{app}}"

# ---------------------------------------------------------------------------
# Installer (.pkg)
# Requires: pkgbuild + productbuild (macOS built-ins via Xcode CLT)
# ---------------------------------------------------------------------------

pkg_version := "0.1.0"
pkg_id      := "com.mevlyshkin.menubar-countdown"
pkg_root    := "target/pkg-root"
component   := "target/release/menubar-countdown-component.pkg"
installer   := "target/release/menubar-countdown-" + pkg_version + ".pkg"

# Build the app bundle and package it into a macOS installer (.pkg)
installer: bundle
    # Populate a staging root: app goes into /Applications
    rm -rf "{{pkg_root}}"
    mkdir -p "{{pkg_root}}/Applications"
    cp -R "{{app}}" "{{pkg_root}}/Applications/"
    # Build the component package
    pkgbuild \
        --root "{{pkg_root}}" \
        --identifier "{{pkg_id}}" \
        --version "{{pkg_version}}" \
        --install-location "/" \
        "{{component}}"
    # Wrap into a distributable installer
    productbuild \
        --package "{{component}}" \
        "{{installer}}"
    rm -f "{{component}}"
    rm -rf "{{pkg_root}}"
    @echo "Created {{installer}}"

# Remove installer artifacts
installer-clean: bundle-clean
    rm -f "{{component}}" "{{installer}}"
    rm -rf "{{pkg_root}}"
