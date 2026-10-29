# LUMA · Horizon на Rust

LUMA — лаунчер для Windows с русским и английским интерфейсом, настройками, проверкой файлов, локальными модами и синхронизацией скинов через компьютер хоста.

Этот репозиторий содержит каталог Orbital Laser и инструменты сборки совместимого игрового клиента. Архив самого лаунчера LUMA 08 распространяется владельцем сборки отдельно.

## Orbital Laser

1. Закройте игру и откройте вкладку **Моды** в LUMA 08.
2. Нажмите **Обновить каталог**, затем **Скачать мод**.
3. После установки включите галочку **Orbital Laser** и запустите игру.

Пакет включает готовый Windows-клиент: устанавливать Rust или средства сборки не требуется. Выключение галочки возвращает обычный клиент, сохраняя скачанный пакет. Первый запуск с модом может занять несколько минут.

В игре: `/orbital give` выдаёт указатель, `/orbital plasma` выбирает лазер. Удерживайте правую кнопку мыши с указателем, направив его на местность. `/orbital terrain on` и `/orbital terrain off` управляют изменением блоков.

## Игра с друзьями

У каждого должен быть включён один и тот же пакет Orbital. Хост открывает мир для сети и разрешает команды гостям, которые будут вызывать удары. Друг подключается к VPN IPv4-адресу хоста и порту мира, например через Radmin/Hamachi. Разный мод или версия вызывает отказ при подключении. Одновременно выполняется один общий удар.

Сервис скинов остаётся на компьютере хоста. Сохраните прежние приглашения и адрес сервиса; изменённые скины видны после перезапуска игры.

Проверки используют два отдельных игровых процесса на Windows. Проверка двух физических ПК через VPN в этот набор тестов не входит. Для важных миров перед использованием изменения местности сделайте резервную копию.

## Источники

- [Horizon fork](https://github.com/Stink-O/horizon/tree/cbceb3f0e8dbd2eba25d3ba0c826338c93e35226), закреплённый исходный коммит.
- [Orbital Laser 0.5.3 от Stink-O](https://github.com/Stink-O/orbital-laser/releases/tag/v0.5.3), оригинальный компонент и ресурсы с сохранёнными лицензиями.
- `release/network/` — собственная сетевая интеграция LUMA.

Ресурсы Minecraft каждый игрок подготавливает отдельно; они не распространяются в пакете мода. Подробности публикации — [RELEASES-RU.md](RELEASES-RU.md).

## English

In LUMA 08, open **Mods**, refresh the catalog, download Orbital Laser and enable its checkbox before starting the game. Every player needs the same enabled package. The host opens their world to LAN and allows guest commands. Join the host VPN IPv4 address and world port. One shared strike runs at a time. Skins continue to synchronize through the host PC service.

The package contains the native Windows client; Rust build tools are not required. Minecraft resources are prepared separately. Local acceptance checks use two game processes; two physical VPN-connected computers were not available for these tests.
