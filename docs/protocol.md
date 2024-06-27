# Protocol Overview

Soaprun consists of two components: an HTTP "dispatch" server and a proprietary binary protocol server.

![Connection](http://www.plantuml.com/plantuml/proxy?cache=no&src=https://raw.githubusercontent.com/CaveStoryModdingCommunity/soapdispenser/main/docs/connection.wsd)

# Dispatch

Soaprun used a CGI server hosted at http://hpcgi2.nifty.com/rochet/en_0x_kb/server.cgi to host metadata about the server, such as whether it was online, the ip/port of the binary server, and, if the server is closed, some comments as to why.

Soaprun starts by opening a TCP connection to the above url on port 80, then sends the following request:
```http
GET /rochet/en_0x_kb/server.cgi HTTP/1.1
Host: hpcgi2.nifty.com
User-Agent: Soaprun1
```

According to [reports when the server was active](https://forum.cavestory.org/threads/i-really-really-think-pixel-is-making-a-new-game.2056/page-25#post-89592), the response would've looked something like this:

```http
HTTP/1.1 200 OK
...

<html><body>
Pixel<br>
110205-222220	open	218.226.167.227	1002	Soaprun	64	.	<br>
</body></html>
```

While this may look like a standard HTTP 1.1 exchange, don't be fooled,
**Soaprun does NOT understand HTTP.**

Soaprun will be able to read the response IF AND ONLY IF it matches this regex:
```
.*(<html).*(?:\r\n|\r|\n)(Pixel).*(?:\r\n|\r|\n).*\t+(.+)\t+(.+)\t+(\d+)\t+(Soaprun)\t+(\d+)(?:\t+(.+)){0,6}
```

Note: some versions of Soaprun may also check for `OK.` to be on the line following `Pixel`, but this functionality is disabled in the latest version.

Put more visually, this is an equally valid response for the server to send:
```
I love Cave Story!
3D! <html
Pixel
IGNORED	open	218.226.167.227	1002	Soaprun	64	comment0	comment1	comment2	comment3	comment4	comment5
According to all known laws
of aviation,

there is no way a bee
should be able to fly.
...
```

Either way, here's how Soaprun interprets the response data:
```ini
status = open
ip = 218.226.167.227
port = 1002
protocol = Soaprun
version = 64
comments = [comment0, comment1, comment2, comment3, comment4, comment5]
```
If the status is something other than `open`, then all found comments will be printed in the log on the right of the game.

Any comments that are exactly equal to `<br>` will not be printed in game, instead leaving a blank line.

Otherwise, Soaprun checks that the protocol is `Soaprun` and the version is `64` (as of latest release).
If both of those pass, then it moves on to the second phase.

# Game Server

Soaprun uses a proprietary binary protocol to handle the actual game communication.


## Packet Definitions

For all code snippets below, `int` represents a 32-bit integer, `short` is a 16 bit integer, `byte` is a one byte integer, `char` is a one byte ASCII character, and all integers are little endian.
The sign of most variables is unknown/untested, but a few are explicitly `signed`.


All Soaprun packets follow the same rough format:

```cs
struct SoaprunPacket
{
    int Length;
    char[4] Type;
    byte[Length-4] Data;
}
```
Therefore, all valid packets have a length of at least 4.

---

### Welcome - "WLCM"
The server must send this packet to the client immediately after a connection is created, otherwise the client will be stuck in limbo forever...

```cs
struct WelcomePacket
{
    int Length = 4;
    char[4] Type = "WLCM";
}
```

### Void - "Void"
Used by the server as a response to certain packets, usually those that result in the client's game ending.

```cs
struct VoidPacket
{
    int Length = 4;
    char[4] Type = "Void";
}
```

### Protocol Information - "Prtc"
Used at the beginning of a connection to ensure the server is actually meant for Soaprun, and that the version matches what the client expects.

```cs
struct ProtocolRequest
{
    int Length = 4;
    char[4] Type = "Prtc";
    short GameVersion; //For v0.432, this should be 432
}
struct ProtocolResponse
{
    int Length = 14;
    char[4] Type = "Prtc";
    char[8] Protocol = "Soaprun\0"; //Note the null terminator
    short Version; //Latest version expects 64
}
```

### Connection Test - "Test"
Used at the beginning of a connection to determine connection speed.
The contents of `Data` are never checked against anything.

```cs
struct ConnectionTestPacket
{
    int Length = 512;
    char[4] Type = "Test";
    char[508] Data;
}
```

### Server Debug Log - "Dlog"
Used by the client to tell the server to log a debug message.
In practice, these messages only contain ASCII characters, but knowing Pixel they could very easily be Shift-JIS.

```cs
struct DebugLogPacket
{
    int Length = 8 + MessageLength;
    char[4] Type = "Dlog";
    int MessageLength; //Literally the output of strlen(Message)
    char[MessageLength] Message; //Your message here
}
```

### Map Attributes - "mAtt"
Used to transmit the tileset's attributes.
This is sent once during initialization.

```cs
struct MapAttributesRequest
{
    int Length = 4;
    char[4] Type = "mAtt";
}
struct MapAttributesResponse
{
    int Length = 8 + (Width * Height);
    char[4] Type = "mAtt";
    short Width;
    short Height;
    byte[Width*Height] Attributes;
}
```

According to a report from when the server was live, the original response was something like this
```cs
struct MapAttributesResponse
{
    int Length = 40;
    char[4] Type = "mAtt";
    short Width = 16;
    short Height = 2;
    byte[32] Attributes = [
        //Three bytes apparently changed sometimes, though the circumstances behind that are unclear
        // 02    02 03
        // v     v  v
        00 01 00 03 02 00 01 03 01 01 01 00 00 02 02 02
        00 01 00 03 02 00 01 03 01 01 01 00 00 02 02 02
    ];
}
```

Here's what each tile type means:
```
0 = Ground
1 = Wall
2 = Player only / No npc
3 = Npc only / No players
```

### Room Data - "Room"
Used to transmit tiles for the requested room.
The client appears to keep a 3x3 buffer of rooms loaded at all times, so `Room` requests will usually be targeted 2 away from the player's current room/1 away from the room they're moving into, unless they're logging in for the first time and need to fill the buffer.

The room width/height is hardcoded in the client (21x16) and it expects each `Room` request to fill that buffer entirely.

Note that room coordinates are SIGNED.

```c
struct RoomRequest
{
    int Length = 6;
    char[4] Type = "Room";
    signed byte X;
    signed byte Y;
}
struct RoomResponse
{
    int Length = 6 + (21*16);
    char[4] Type = "Room";
    signed byte X;
    signed byte Y;
    byte[21*16] Tiles;
}
```

### Player Position - "myPo"
Used by the client to send their position to the server.
Each packet contains a list of movement nodes that represent where the client has moved since the last time it communicated with the server.

Despite the fact that the map is split up into rooms, all positions use GLOBAL coordinates and are SIGNED.

Many packets reuse this same format for sending movement nodes, so a lot of the server's time will be spent parsing/verifying them.

```c
struct Position
{
    signed short X;
    signed short Y;
}
struct PositionPacket
{
    int Length = 5 + (MovementsLength*sizeof(Position));
    char[4] Type = "myPo";
    byte MovementsLength;
    Position[MovementsLength] Movements;
}
```

### Field Data - "Flds"
This is what the server sends to the client in response to 99% of packets.

It contains:

- Client Information
    - State
    - Color
    - Items
- Weather
- Player Information
    - Position
    - Items
    - Movements
- NPC Information
    - Position
    - Direction (where applicable)
    - Movements
- Modified Tiles
    - Position
    - Type

Soaprun's system for determining entity type is weird.
Unlike Cave Story, which uses a single integer to index into a list of types, Soaprun uses X/Y coordinates based off the top left corner of the relevant spritesheet (`funit-npu.bmp` for npcs, `funit-pla.bmp` for other players).
Since everything in Soaprun has two sprites that they cycle between, the X value can be thought of as being multiplied by 2, but the Y axis is normal.

For example, an NPC with type (1,0) would correspond to the Goal flag, while (1,2) would be the Sword, and (0,11) would be a sleeping Snail.
For players, (0,0) would be a green Soaprunner standing still, while (3,2) would be a blue Soaprunner winning.

Note that soaprunner/entity indexes are expected to be provided in ASCENDING ORDER.

```cs
struct FieldPacket
{
    int Length; //At least 11, but the exact number requires knowing how many movements each individual player/npc made
    char[4] Type = "Flds";
    SoaprunnerSprites ClientState;
    SoaprunnerColors ClientColor;
    SoaprunnerItems ClientItems;
    byte SoaprunnerCount;
    byte EntityCount;
    byte TileCount;
    Weather CurrentWeather;
    SoaprunnerData[SoaprunnerCount] Soaprunners;
    EntityData[EntityCount] Entities;
    TileData[TileCount] Tiles;
}
enum SoaprunnerSprites : byte
{
    Idle = 0, //not seen in any existing footage, seems to be intended for AFK players
    Walking,
    Dying,
    Winning,
    Ghost
}
enum SoaprunnerColors : byte
{
    Green = 0,
    Pink,
    Blue,
    Yellow
}
//This one's a bitfield
enum SoaprunnerItems : byte
{
    Sword = 1,
    Crown = 2,
    Shield = 4
}
enum Weather : byte
{
    Clear = 0,
    Rainy = 1
}
struct SoaprunnerData
{
    byte Index;
    byte TeleportTrigger;
    SoaprunnerSprites Sprite;
    SoaprunnerColors Color;
    SoaprunnerItems Items;
    byte MovementsLength;
    Position[MovementsLength] Movements;
}
//Some of these state names don't make sense for the inanimate objects, but...
enum EntityStates : byte
{
    Sleeping = 0,
    Active,
    Corpse,
    Flickering,
    Gone
}
//Values are official names from the exe, comments are names taken from Soaprules.rtf
enum EntityTypes : byte
{
    Goal = 0,
    Closer,  //Black Demon
    Sword,
    Crawl,   //Guard Demon
    Hummer,  //Red Flame
    Rounder, //Blue Flame
    Wuss,    //Blue Demon
    Chase,   //Crawling Demon
    Gate,    //Purple Flame
    Shield, 
    Cross,   //Green Flame
    Snail
}
struct EntityData
{
    byte Index;
    byte TeleportTrigger;
    EntityStates State;
    EntityTypes Type;
    byte Direction; //Only used by Gates and Crosses
    byte MovementsLength;
    Position[MovementsLength] Movements;
}
struct TileData
{
    signed short X;
    signed short Y;
    byte Type;
    byte Padding; //Something something struct alignment
}
```

### Change Color - "ChCl"
Used by the client to change their Soaprunner's color.

The contents should be verified to make sure clients don't try to turn invisible using an OOB color index.

Also note that this packet includes the same movement information as the `myPo` packet.

```c
struct ChangeColorPacket
{
    int Length = 6 + (MovementsLength*sizeof(Position));
    char[4] Type = "ChCl";
    unsigned byte Color;
    byte MovementsLength;
    Position[MovementsLength] Movements;
}
```

### Draw On Field - "DrFl"
Used by the client when drawing on a drawing tile (tiles 12,13,14,15).

As of v0.432, the client only tries to draw using those tiles, even if there's a corpse on the tile, so it's up to the server whether any corpses should be cleared or preserved when drawn over.

It is recommended for server implementations to verify the data in this packet to prevented hacked/custom clients from overwriting the entire map.

Again, notice the inclusion of the same movement information as the `myPo` packet.

```cs
struct DrawPacket
{
    int Length = 10 + (MovementsLength*sizeof(Position))
    char[4] Type = "DrFl";
    signed short X;
    signed short Y;
    byte TileType;
    byte MovementsLength;
    Position[MovementsLength] Movements;
}
```

### Hit Non-Player Unit - "HNPU"

Sent by the client when it detects collision with an entity.

Verifying the contents of this packet is very difficult since entity/soaprunner movement is handled client side, so the client can easily collide with an entity the server thinks has already passed them by.
Obviously this isn't a problem for static items, but maybe don't try to verify flame collisions very hard.

Oh look, it's the same movement information as the `myPo` packet.

```cs
struct HitNonPlayerUnitPacket
{
    int Length = 6 + (MovementsLength*sizeof(Position));
    char[4] Type = "HNPU";
    byte CollidedEntityIndex;
    byte MovementsLength;
    Position[MovementsLength] Movements;
}
```

### Go To Heaven - "HVen"
Sent by the client when it collides with a (real/non-player) ghost.
Signals to the server that this client should be turned into a ghost.

Everything to do with non-player ghosts is handled client-side, so there isn't a good way to prevent hacked/custom clients from never turning into a ghost.
Maybe if someone hasn't turned into a ghost after 30 min, idk.

Ghosts do indeed move, so they too include the same movement information as the `myPo` packet.

```cs
struct GoToHeavenPacket
{
    int Length = 5 + (MovementsLength*sizeof(Position));
    char[4] Type = "HVen";
    byte MovementsLength;
    Position[MovementsLength] Movements;
}
```

### Make Corpse - "mCrp"

Sent by the client after a few seconds of being in the "Dying" state, which should occur when it dies to a regular enemy.
Signals for the server to make a corpse on the position the client died.

Verifying the contents of this packet to prevent hacked clients from drawing with corpses all over the map.

```cs
struct MakeCorpsePacket
{
    int Length = 8;
    char[4] Type = "mCrp";
    signed short X;
    signed short Y;
}
```

### Disconnect - "Bye."
Sent by the client when it disconnects after winning or dying.

Note that it is NOT sent when the client disconnects by closing/resetting the game, that just closes the network connection.

```cs
struct DisconnectPacket
{
    int Length = 4;
    char[4] Type = "Bye.";
}
```