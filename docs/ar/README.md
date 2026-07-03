<p align="center">
  <img src="../../res/logo.webp" alt="Aoba Logo" width="240" />
</p>

<p align="center">
  <h1 align="center">Aoba</h1>
</p>

<p align="center">
  <a href="https://github.com/celestia-island/aoba/actions/workflows/checks.yml">
    <img src="https://github.com/celestia-island/aoba/actions/workflows/checks.yml/badge.svg?branch=master" alt="Checks status" />
  </a>
  <a href="https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-tui.yml">
    <img src="https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-tui.yml/badge.svg?branch=master" alt="E2E TUI status" />
  </a>
  <a href="https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-cli.yml">
    <img src="https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-cli.yml/badge.svg?branch=master" alt="E2E CLI status" />
  </a>
  <a href="../../LICENSE">
    <img src="https://img.shields.io/badge/license-SySL%201.0-blue" alt="License: SySL" />
  </a>
  <a href="https://github.com/celestia-island/aoba/releases/latest">
    <img src="https://img.shields.io/github/v/tag/celestia-island/aoba?label=version&sort=semver" alt="Latest Version" />
  </a>
</p>

<p align="center">
  AR | <a href="../en/README.md">EN</a> | <a href="../es/README.md">ES</a> | <a href="../fr/README.md">FR</a> | <a href="../ja/README.md">JA</a> | <a href="../ko/README.md">KO</a> | <a href="../ru/README.md">RU</a> | <a href="../zhs/README.md">ZH</a> | <a href="../zht/README.md">ZHT</a>
</p>

أداة تصحيح ومحاكاة متعددة البروتوكولات لـ Modbus RTU، مناسبة للمنافذ التسلسلية الفعلية ومنافذ إعادة التوجيه عبر الشبكة. توفر واجهتي CLI و TUI.

## الميزات

- تصحيح ومحاكاة Modbus RTU (رئيسي/تابع)؛ يدعم أربعة أنواع من السجلات: holding و input و coils و discrete.
- واجهة CLI كاملة الميزات: اكتشاف المنافذ وفحصها (`--list-ports` / `--check-port`)، عمليات رئيسي/تابع (`--master-provide` / `--slave-listen`) والأوضاع المستمرة (`--*-persist`). يمكن أن تكون المخرجات بصيغة JSON/JSONL مناسبة للبرامج النصية وCI.
- TUI تفاعلي: تكوين المنافذ والمحطات والسجلات عبر واجهة طرفية؛ يدعم الحفظ/التحميل (`Ctrl+S` يحفظ ويفعل المنافذ تلقائياً) وتكامل IPC مع CLI للاختبار والأتمتة.
- مصادر بيانات وبروتوكولات متعددة: منافذ تسلسلية فعلية/افتراضية (تُدار عبر `socat`), HTTP, MQTT, IPC (مقابس Unix / أنابيب مسماة)، ملفات، و FIFOs.
- إعادة توجيه المنافذ: تكوين منافذ المصدر والهدف داخل TUI لنسخ البيانات أو المراقبة أو التجسير.
- وضع الخادم: التشغيل بدون واجهة باستخدام تكوين TUI محفوظ لتشغيل جميع المنافذ/المحطات المكونة (مناسب للنشر المضمن/CI).
- أدوات المنافذ الافتراضية والاختبار: يتضمن `scripts/socat_init.sh` للمنافذ التسلسلية الافتراضية واختبارات أمثلة في `examples/cli_e2e` و `examples/tui_e2e`.

> ملاحظة: استخدم `--no-config-cache` لتعطيل حفظ/تحميل TUI؛ `--config-file <FILE>` و `--no-config-cache` متعارضان.

## بداية سريعة

1. تثبيت سلسلة أدوات Rust
2. استنساخ المستودع والدخول إلى المجلد
3. التثبيت:
   - البناء من المصدر: `cargo install aoba`
   - أو تثبيت إصدار مبني بواسطة CI (إذا كان متاحاً) باستخدام `cargo-binstall`:
     - مثال: `cargo binstall --manifest-path ./Cargo.toml --version <version>`
     - استخدم `--target <triple>` لاختيار منصة محددة (مثل `x86_64-unknown-linux-gnu`)
4. تشغيل `aoba` لبدء TUI افتراضياً

## توثيق

- [API Modbus Master](API_MODBUS_MASTER.md)
- [API Modbus Slave](API_MODBUS_SLAVE.md)
- [CLI Modbus](CLI_MODBUS.md)
- [مصدر بيانات HTTP](DATA_SOURCE_HTTP.md)
- [مصدر بيانات IPC](DATA_SOURCE_IPC.md)
- [مصدر بيانات MQTT](DATA_SOURCE_MQTT.md)
- [إعادة توجيه المنافذ](DATA_SOURCE_PORT_FORWARDING.md)

## اللغات

| اللغة | الرابط |
|--------|------|
| العربية | [AR](README.md) |
| English | [EN](../en/README.md) |
| Espanol | [ES](../es/README.md) |
| Francais | [FR](../fr/README.md) |
| 日本語 | [JA](../ja/README.md) |
| 한국어 | [KO](../ko/README.md) |
| Русский | [RU](../ru/README.md) |
| 简体中文 | [ZH](../zhs/README.md) |
| 繁體中文 | [ZHT](../zht/README.md) |

## الترخيص

مرخص بموجب [Synthetic Source License (SySL), Version 1.0](../../LICENSE).
