#!/bin/sh
# Install only the TUI. Existing user config and MPD gateway remain in place.
set -eu
source_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
source_binary="$source_dir/mopytui"
if [ ! -f "$source_binary" ] || [ ! -x "$source_binary" ]; then
    echo 'Поместите install-system.sh рядом с бинарником mopytui из архива v15.' >&2
    exit 1
fi
if [ "$(id -u)" -ne 0 ]; then
    exec sudo sh "$source_dir/install-system.sh"
fi
install -d -m 755 /usr/local/bin
if [ -e /usr/local/bin/mopytui ] || [ -L /usr/local/bin/mopytui ]; then
    backup_dir=$(mktemp -d /usr/local/bin/.mopytui-backup.XXXXXXXX)
    cp -a /usr/local/bin/mopytui "$backup_dir/mopytui"
    printf 'Предыдущая версия сохранена: %s/mopytui\n' "$backup_dir"
fi
temporary=$(mktemp /usr/local/bin/.mopytui-install.XXXXXXXX)
trap 'rm -f -- "$temporary"' EXIT HUP INT TERM
install -m 755 "$source_binary" "$temporary"
mv -fT "$temporary" /usr/local/bin/mopytui
printf '%s\n' 'Установлено: /usr/local/bin/mopytui' 'Запускайте mopytui обычным пользователем, без sudo.' 'Используется ваш прежний ~/.config/mopytui/config.toml, включая [radio].proxy.' 'Шлюз mpc и настройки Mopidy не изменены.'
