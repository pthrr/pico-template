{
  description = "Pico Template";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" "rustfmt" "clippy" ];
          targets = [
            "thumbv6m-none-eabi"        # Pico 1 (RP2040 - Cortex-M0+)
            "thumbv8m.main-none-eabihf" # Pico 2 (RP2350 - Cortex-M33) & STM32U585 (Cortex-M33 with FPU)
          ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            openocd
            probe-rs
            elf2uf2-rs
            flip-link
            picotool
            arduino-cli
            gcc-arm-embedded  # for arm-none-eabi-objcopy
            jq                # for JSON parsing in tasks
            cue               # build.rs platform config export

            uv
            ruff
            go-task
            picocom
            mcrl2
          ];

          shellHook = ''
            echo "Rust toolchain: ${rustToolchain}"
            # Check if Arduino Zephyr core is installed for UNO Q support
            if [ ! -d "$HOME/.arduino15/packages/arduino/hardware/zephyr" ]; then
              echo "Note: Run 'task setup_unoq' to install Arduino Zephyr core for UNO Q flashing"
            fi
          '';

          RUST_BACKTRACE = "1";
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";

          OPENOCD_SCRIPTS = "${pkgs.openocd}/share/openocd/scripts";

          # USB permissions
          # For Pico (RP2040/RP2350):
          #   Add user to plugdev group: sudo usermod -a -G plugdev $USER
          #   Create udev rule: /etc/udev/rules.d/99-pico.rules
          #   SUBSYSTEM=="usb", ATTRS{idVendor}=="2e8a", MODE="0666"
          # For STM32 (ST-Link):
          #   SUBSYSTEM=="usb", ATTRS{idVendor}=="0483", MODE="0666"
        };

        formatter = pkgs.nixpkgs-fmt;

        checks = {
          format-check = pkgs.runCommand "format-check" {
            buildInputs = [ pkgs.nixpkgs-fmt ];
          } ''
            nixpkgs-fmt --check ${self}
            touch $out
          '';
        };
      }
    );
}
