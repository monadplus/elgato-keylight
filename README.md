# Elgato Key Light Controller

CLI controller for Linux written in Rust

> GUI coming soon..

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

To discover the IP of your Elgato Key Light you can use:

```sh
$ elgato-keylight-discover
[
    Resolved {
        base: MdnsPacketBase {
            interface_name: "enp1s0",
            internet_protocol: V4,
            hostname: "Elgato Key Light 88DD",
            service_type: "_elg._tcp",
            domain: "local",
        },
        service: Service {
            name: "_elg._tcp",
            hostname: "elgato-key-light-8d7c.local",
            ip: 192.168.1.100,
            port: 9333,
            data: [
                "\"pv=1.0\" \"md=Elgato Key Light 20GAK9901\" \"id=FF:6A:9D:30:B1:6E\" \"dt=53\" \"mf=Elgato\"",
            ],
        },
    }
]
```

## Dependencies

* `avahi-browse`: for device discovery
* (optional) `notify-send`: for desktop notifications
