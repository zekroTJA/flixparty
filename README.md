# flixparty

A very simple and janky tool to synchronize play-pause events across different PCs to watch video 
content together on platforms, which do not support watch parties.

> [!WARNING]  
> Because this app is basically a keylogger, you might run into some antivirus issues when 
> downloading the pre-compiled binaries. If this is the case, you might want to compile the
> client locally on the target system.

## How does it work?

Via the config, every client connects to a common Redis pub-sub channel. When a client detects a 
key-press on the configured toggle-key (defaulty <kbd>Space</kbd>), an event is published in the
pub-sub channel. All other clients listen to the toggle-event and will, when received, issue a
"pause simulation" on the local machine. This means the cursor is moved to the center of the main
screen, then a left mouse button press and release is issued - which should pause the playback -, 
and after that, the cursor is moved back to the original location.

**Why using mouse clicks to pause and unpause?** Sadly, rdev - the library used to listen to
keyboard events, can not distinguish between real and simulated keyboard events. So this would
end in an infinite feedback loop between all clients when the sender-key would also be simulated
on the other clients.  

## Setup

First of all, you need to set up a public Redis instance where the clients connect to. Either, you
can self-host Redis on your own infrastructure, or you can simply use free tier alternatives like
from [redis.io](https://redis.io/).

Then, [download](https://github.com/zekroTJA/flixparty/releases) or build the client application.

After that, you need a config file. You can simply use the provided
[example config](flixparty.config.toml) as a starting point. Defaultly, the client looks for a
`flixparty.config.toml` in the current working directory (the directory where you launch the app
from). Alternatively, you can pass a path to a config file, if you want to store it somewhere else.

```
./flixparty path/to/my/config.toml
```

In the config, set the address of your redis instance as `address` in the `connection` block. Also, 
set the name of the channel to be used for communication between the clients. This can be any 
arbitrary string, but it must be the same one for all of the clients who should be connected. This
way, you can also use the same Redis instance for multiple sessions by using different channel 
names, if you want.