# Elgato Key Light Controller

## Installation

```sh
cargo install --path . --force
```

## Usage

```sh
$ elgato-keylight --help
Elgato Keylight controller

Usage: elgato-keylight --host <HOST> --port <PORT> <COMMAND>

Commands:
  status            Status: on/off, brightness, temperature, etc
  toggle            Toggle (on/off)
  incr-brightness   Increase brightness by 10%
  decr-brightness   Decrease brightness by 10%
  incr-temperature  Increase temperature by 10%
  decr-temperature  Decrease temperature by 10%
  set               Set values for brightness and temperature
  help              Print this message or the help of the given subcommand(s)

Options:
      --host <HOST>  IP address
      --port <PORT>  API port
  -h, --help         Print help
  -V, --version      Print version
```

To discover the IP of your key light you can use:

```sh
avahi-browse -t _elg._tcp --resolve
```

## Dependencies

* (optional) `notify-send`

## Coming soon

* GUI
* Device discovery
