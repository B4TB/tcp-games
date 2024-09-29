# cat library
i think this is a pager with Line Editing Capabilities attached to terrible cat twitter

## quick start
```console
$ cargo run --release
```

will listen on port 6868 (hardcoded) over TCP and provide access to the Cat Library.
memory is entirely ephemeral and is Abandoned when the process dies (rest in peace).

you can connect like this if you want to, replacing localhost with address of the server it's running on:
```console
$ nc localhost 6868
```
