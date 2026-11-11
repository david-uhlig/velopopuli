**VeloPopuli: the people's indoor ride.** Ride real GPX routes in 3D on your smart trainer. Free and open source. [velopopuli.com](https://velopopuli.com) · [Guides](https://github.com/pepperstm/velopopuli/wiki)

> This is an early release. The 3D world, physics, workouts and progress tracking are well tested; real trainers and sensors are only now being tried, and the Windows and Linux builds have had little use. If something doesn't work, the app's **Report a problem → Report on GitHub** builds a diagnostics report for you. Please send one.

## What's new since 0.3.10

- **Heart-rate mode.** Press **Heart rate** on the ride screen and the trainer sets the power to keep your heart rate in a band — good for long, steady rides. It starts with a gentle warm-up, changes the power in small steps, eases off as your heart rate drifts up over a long ride, and backs off if your legs give out. Move the band 2 bpm at a time with the arrow keys, the on-screen buttons or a handlebar controller. Set the band, the power limits and the warm-up under **Settings → Heart-rate mode**, or leave them to be worked out from your max heart rate and FTP. It works on rides without a workout, with a heart-rate strap and a trainer VeloPopuli can control. See [Heart-rate pacing](https://github.com/pepperstm/velopopuli/wiki/Heart-rate-pacing).
- **A more lifelike demo rider.** The demo rider's heart now answers effort late and slowly and drifts up on long efforts, like a real one, so you can try heart-rate mode with no hardware.
- Heart-rate mode is new and hasn't been tried with a real strap and trainer yet. It is a training aid, not medical advice: if you feel unwell, stop riding. If it misbehaves, **Report a problem** in the app gathers what's needed to fix it.
- Going back to an older version? Switch the ride screen to **Gradient** or **ERG** first: older versions don't know heart-rate mode and would ride with default settings.

## What's new in 0.3.10

- **Laps.** Ride a loop route as many times as you like. The app counts laps and distance as you go and times every climb on every lap. Thanks to Rallensooo for adding it.
- **Herne Hill Velodrome.** The outdoor track in south London where the 1948 Olympic track races were held comes built in: 450 m round, banked into the bends, with the pavilion, stands and floodlights. Ride it anticlockwise, as they race it, or the other way round.
- **Find signposted road routes.** Find a route now shows the signposted cycle routes from OpenStreetMap on the map, alongside the mountain-bike trails and famous climbs.
- **Your Strava routes.** With Strava connected, the routes you've saved there are listed in Find a route, ready to ride. Reconnect Strava once to see your private routes.
- **Dreeve.** Send rides to your own [Dreeve](https://github.com/pepperstm/velopopuli/wiki/Dreeve) server, race your Dreeve rides as ghosts, and turn any of them into a route (Dreeve 5.4 or later). Thanks to Jörn for adding it, and for making services like Strava and Dreeve pluggable.
- **Sensors are remembered.** Trainers, power meters and heart-rate straps you've paired connect by themselves when the app starts, and Devices shows which ones it's still looking for.
- **Smoother bends.** The rider follows a smooth curve through corners instead of turning in steps, and leans into the bends.
- **Linux Bluetooth.** Sensors that wouldn't connect on Linux (BlueZ cancelling the connection) should now connect: the app stops scanning while it connects.
- Ending a ride too short to save now says so, and a ghost on a loop route starts from the start.

## Download

| System | File | Notes |
|---|---|---|
| **macOS** (Apple Silicon and Intel, 10.15+) | `VeloPopuli_*_universal.dmg` | Drag to Applications. Not notarised yet: the first time, open **System Settings → Privacy & Security → Open Anyway** (or run `xattr -dr com.apple.quarantine /Applications/VeloPopuli.app`). |
| **Windows** 10/11 (64-bit) | `VeloPopuli_*_x64-setup.exe` or `VeloPopuli_*_x64_en-US.msi` | Not code-signed yet: if SmartScreen warns, choose **More info → Run anyway**. ANT+ sticks need the WinUSB driver ([Zadig](https://zadig.akeo.ie)). |
| **Ubuntu / Debian** (and Mint, Pop!_OS…; 64-bit) | `VeloPopuli_*_amd64.deb` | `sudo apt install ./VeloPopuli_*_amd64.deb`. It installs the ANT+ stick permission rule too. |
| **Arch Linux** (and Manjaro, EndeavourOS…) | `velopopuli-bin-*-x86_64.pkg.tar.zst` | `sudo pacman -U velopopuli-bin-*.pkg.tar.zst`. The `PKGBUILD` is attached too. Install `bluez` for Bluetooth. |
| **Any other Linux** (64-bit) | `VeloPopuli_*_amd64.AppImage` | `chmod +x` and run. Later versions can be installed with AppImageUpdate, which downloads only what has changed. |

**Group-ride relay** (optional, for hosting rides with friends): `velopopuli-relay-*` for macOS, Windows and Linux; see [Group rides and self-hosting](https://github.com/pepperstm/velopopuli/wiki/Group-rides-and-self-hosting).

Setting up: [Installing](https://github.com/pepperstm/velopopuli/wiki/Installing) · [Connecting your trainer](https://github.com/pepperstm/velopopuli/wiki/Connecting-your-trainer) · [Your first ride](https://github.com/pepperstm/velopopuli/wiki/Your-first-ride)
