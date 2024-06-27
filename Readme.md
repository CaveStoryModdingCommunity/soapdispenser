# Soapdispenser

It's a replacement server for [Soaprun](https://www.cavestory.org/pixels-works/soaprun.php) written in Rust!

See the [docs folder](docs) for more info on Soaprun itself.

# Building

It's a rust project, so just `cargo build` and it should work.

# Running

To start the server, set up your [config.json] and run the executable.
If you want to specify a specific config file, such as when hosting multiple servers, use the `-c` (`--config`) option.
See the [recreations folder](recreations) for some maps you can host.

If you want to convert legacy maps (Soaprun version 0.020, 0.030, or any of the offline executables), use this command:
```
soapdispenser.exe ConvertRooms <input directory> <conversion map> [output directory (pulls from config.json if not provided)]
```
Some conversion maps can be found in the `conversion_maps` folder (they're just a list of bytes where each tile type is used as an index into the file to find what tile it should be in the final output).


# Credits
- Pixel - Made Soaprun
- Brayconn - Made this replacement server
- Hamish "hammil" Milne - Provided early documentation of the Soaprun protocol
- andwhyisit - Provided old videos, maps, and server space at [soaprun.cavestory.org]
- Enlight - Helped test the server, and made the Soaprun Community Edition client
- Satwon - Helped test the server
- Fluff8836 - Made the offline map viewers, Soaprules.rtf, multiple map images, and the only English Soaprun footage