# mspm0l222x-hal

[![Crates.io Version](https://img.shields.io/crates/v/mspm0l222x-pac)](https://crates.io/crates/mspm0l222x-pac)
[![docs.rs](https://img.shields.io/docsrs/mspm0l222x-pac)](https://docs.rs/mspm0l222x-pac)

> [!CAUTION]  
> This is a placeholder crate as development on the HAL is still in its early stages. Do not attempt to use this crate.

This is an [Embedded HAL] (Hardware Abstraction Layer) for the MSPM0L2228 microcontroller from Texas Instruments.

The HAL is built on top of the [`mspm0l222x-pac`] Peripheral Access Crate, which provides low-level access to the MSPM0L2228's registers. The HAL provides a higher-level interface to the MSPM0L2228's peripherals, making it easier to write applications.

[Embedded HAL]: https://crates.io/crates/embedded-hal
[`mspm0l222x-pac`]: https://github.com/sigpwny/mspm0l222x-pac

> [!NOTE]  
> This HAL is under active development. As a result, the API is volatile and subject to change. Be sure to refer to the [changelog] for breaking changes.

If you want updates for when new releases are made, you can watch this repository by clicking the "Watch" button at the top of the page.

[changelog]: https://github.com/sigpwny/mspm0l222x-hal/releases

## Documentation
Documentation for this HAL can be built by running:
```sh
cargo doc --open
```

Documentation can also be found on [docs.rs](https://docs.rs/mspm0l222x-hal).

## License
This template is licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

Copyright (c) 2025 SIGPwny