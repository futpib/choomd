# choomd

> Adjust process OOM-killer scores based on process names and other attributes

## Install

Arch Linux: https://aur.archlinux.org/packages/choomd-git

## Config

Edit `/etc/choomd.toml` to your liking, for example:

```toml
[rules.firefox-contentproc]
command_line_file_name = [ 'firefox', 'firefox-bin' ]
command_line_argument = [ '-contentproc' ]
oom_score_adj = 450

[rules.firefox]
command_line_file_name = [ 'firefox', 'firefox-bin' ]
oom_score_adj = 500
```

More on configuration here: https://github.com/futpib/choomd/blob/master/etc/choomd.toml

Enable the service:

```bash
systemctl enable --now choomd
```

## Debug or test your config

```bash
RUST_LOG=debug choomd --config-file ./choomd.toml
```

## PSA

Avoid setting negative or low (< 200) `oom_score_adj` as you may create an unkillable monster

## See also

* [`Alt+SysRq+f`](https://wiki.archlinux.org/title/keyboard_shortcuts#Kernel_(SysRq)) kernel hotkey that triggers OOM killer
* [`OOMPolicy`](https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html#OOMPolicy=) and [`DefaultOOMPolicy`](https://www.freedesktop.org/software/systemd/man/latest/systemd-system.conf.html#DefaultOOMPolicy=) systemd options that make systemd terminate siblings of an OOM-killed process
* [`OOM` `htop` column](https://www.man7.org/linux/man-pages/man1/htop.1.html#COLUMNS) to see which processes are next in line for the OOM kill

## What does the name mean, how do you pronounce it?

Change OOM daemon. By analogy with [`choom`](https://man7.org/linux/man-pages/man1/choom.1.html) command.
