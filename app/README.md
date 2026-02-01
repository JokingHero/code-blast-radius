# Tauri + Solid + Typescript
```sh
cd app
npm run tauri dev
```

# Build
```sh
cd app
npm run tauri build
sudo dpkg -i "../target/release/bundle/deb/Code Blast Radius_0.1.0_amd64.deb"
sudo apt remove code-blast-radius

chmod +x "../target/release/bundle/appimage/Code Blast Radius_0.1.0_amd64.AppImage"
"./../target/release/bundle/appimage/Code Blast Radius_0.1.0_amd64.AppImage"
```


# Making this more convenient for devlopment
```sh
codeblast-link() {
  APP_DIR="$HOME/path/to/target/release/bundle/appimage"
  LATEST_APP=$(ls -t "$APP_DIR"/Code\ Blast\ Radius_*_amd64.AppImage | head -n 1)

  if [ -z "$LATEST_APP" ]; then
    echo "No AppImage found"
    return 1
  fi

  chmod +x "$LATEST_APP"
  ln -sf "$LATEST_APP" "$HOME/.local/bin/code-blast-radius"
  echo "Linked -> $LATEST_APP"
}

export PATH="$HOME/.local/bin:$PATH"

codeblast-link
code-blast-radius
``` 

nano ~/.local/share/applications/code-blast-radius.desktop

[Desktop Entry]
Type=Application
Name=Code Blast Radius
Exec=/home/YOUR_USER/.local/bin/code-blast-radius
Icon=utilities-terminal
Terminal=false
Categories=Development;

update-desktop-database ~/.local/share/applications
