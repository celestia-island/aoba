# دليل استخدام واجهة برمجة تطبيقات Modbus الرئيسية (Master)

يصف هذا المستند كيفية استخدام واجهة برمجة تطبيقات Modbus الرئيسية (Master) من Aoba في تطبيقات Rust ضمن سيناريوهات صناعية نموذجية (مراقبة خطوط الإنتاج، التحكم في العمليات، مراقبة البيئة، إلخ)، باستخدام حزمة `examples/api_master` كمرجع.

## 1. نظرة عامة

توفّر Aoba واجهة برمجة تطبيقات Modbus رئيسية قائمة على الـ traits مخصصة للتضمين في تطبيقات Rust الأخرى أو برامج التحكم في الأجهزة. تشمل حالات الاستخدام النموذجية:

- الاستقصاء الدوري لأجهزة Modbus التابعة (RTU عبر منافذ تسلسلية أو افتراضية)
- جمع قيم الملفات (coils) والسجلات (registers) في منطق القياس أو التحكم الخاص بك
- التكامل مع أنظمة التسجيل والمراقبة الموجودة عبر hooks

نقطة الدخول الأساسية هي النوع `ModbusBuilder` من `_main::api::modbus`.

```rust
use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};
```

> ملاحظة: في الأمثلة يُسمّى جذر الحزمة `_main`. في مشروعك الخاص سيكون هذا عادةً الحزمة الرئيسية `aoba` أو أي اسم تطلقه عليها في `Cargo.toml`.

---

## 2. دورة الحياة الأساسية للجهاز الرئيسي

تبدو حلقة الاستقصاء الرئيسية البسيطة كالتالي:

```rust
use anyhow::Result;
use std::time::Duration;
use _main::api::modbus::{ModbusBuilder, RegisterMode};

fn main() -> Result<()> {
    let master = ModbusBuilder::new_master(1) // معرف المحطة للجهاز التابع
        .with_port("/dev/ttyUSB0")          // أو `/tmp/vcom1` إلخ
        .with_register(RegisterMode::Holding, 0, 10)
        .with_timeout(1000)                  // مللي ثانية
        .build_master()?;

    loop {
        if let Some(resp) = master.recv_timeout(Duration::from_secs(1)) {
            println!("values = {:04X?}", resp.values);
        }
    }
}
```

### المعاملات المهمة

- **المنفذ**: أي منفذ تسلسلي أو افتراضي يمكن لـ Aoba فتحه (منفذ حقيقي `/dev/ttyUSB*`، `/dev/ttyS*`، أو منفذ افتراضي `/tmp/vcom*` مُنشأ بواسطة socat).
- **معرف المحطة**: عنوان جهاز Modbus التابع (عادةً 1–247).
- **وضع السجل**: أحد `RegisterMode::Coils`، `DiscreteInputs`، `Holding`، `Input`.
- **عنوان السجل / الطول**: عنوان البداية وعدد العناصر المراد قراءتها، مطابقةً لجدول عناوين Modbus الخاص بجهازك (على سبيل المثال، PLC أو بوابة مستشعرات).
- **المهلة**: مهلة الطلب بالمللي ثانية.

يقوم الجهاز الرئيسي داخليًا بتشغيل حلقة استقصاء ويغذي الاستجابات في قناة؛ يقوم كودك ببساطة باستدعاء `recv_timeout` للحصول على بيانات جديدة.

---

## 3. استخدام hooks للتسجيل والمراقبة

بالنسبة لأنظمة الإنتاج (الخطوط الصناعية، معدات العمليات، المستشعرات الميدانية، إلخ) عادةً ما ترغب في:

- تسجيل كل استجابة ناجحة
- تتبع الأخطاء والمهل
- إمكانية دفع البيانات إلى ناقل رسائل أو قاعدة بيانات

يتيح لك trait `ModbusHook` توصيل هذا المنطق مركزيًا.

```rust
use anyhow::Result;
use std::sync::Arc;
use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};

struct LoggingHook;

impl ModbusHook for LoggingHook {
    fn on_before_request(&self, port: &str) -> Result<()> {
        log::debug!("sending request on {}", port);
        Ok(())
    }

    fn on_after_response(&self, port: &str, resp: &ModbusResponse) -> Result<()> {
        log::info!(
            "resp {}: station={}, addr=0x{:04X}, values={:04X?}",
            port,
            resp.station_id,
            resp.register_address,
            resp.values,
        );
        Ok(())
    }

    fn on_error(&self, port: &str, err: &anyhow::Error) {
        log::warn!("modbus error on {}: {}", port, err);
    }
}

fn main() -> Result<()> {
    env_logger::init();

    let master = ModbusBuilder::new_master(1)
        .with_port("/tmp/vcom1")
        .with_register(RegisterMode::Holding, 0, 5)
        .with_timeout(1000)
        .add_hook(Arc::new(LoggingHook))
        .build_master()?;

    // الآن قم بالاستقصاء باستخدام recv_timeout كما في المثال الأساسي
    # let _ = master;
    Ok(())
}
```

يمكنك تسجيل عدة hooks (على سبيل المثال، واحد للتسجيل، وآخر لتصدير المقاييس).

---

## 4. نمط التكامل لمراقبة الأجهزة الصناعية

بالنسبة لسيناريوهات المراقبة الصناعية النموذجية (خطوط الإنتاج، وحدات العمليات، أجهزة مراقبة البيئة، إلخ)، النمط الشائع هو:

1. **تكوين المنافذ والمحطات** عبر واجهة Aoba TUI أو CLI، أو ترميزها بشكل ثابت في تطبيقك.
2. **إنشاء جهاز رئيسي واحد لكل منفذ فعلي/افتراضي** باستخدام `ModbusBuilder::new_master`.
3. **إنشاء مهمة Tokio لكل جهاز رئيسي** تقوم بـ:
   - استدعاء `recv_timeout` في حلقة
   - تحليل `ModbusResponse::values` إلى وحدات هندسية (ضغط، درجة حرارة، حالة صمام، إلخ)
   - إعادة توجيه البيانات المعالجة إلى الواجهة الخلفية للمراقبة (MQTT، HTTP، قاعدة بيانات، إلخ).
4. استخدام `ModbusHook` لمركزة التسجيل وقياس زمن الانتقال وعدّ الأخطاء.

نظرًا لأن Aoba مبني على `tokio`، فإن واجهة برمجة التطبيقات الرئيسية مصممة للاستخدام داخل بيئة تشغيل غير متزامنة ولكنها تعرض `recv_timeout` بسيطًا بنمط الحظر لسهولة الاستخدام في المهام.

---

## 5. معالجة الأخطاء والمهل

- تُرجع `build_master()` خطأ `anyhow::Error` إذا تعذر فتح المنفذ أو كان التكوين غير صالح.
- تُرجع `recv_timeout()` القيمة `None` عند انتهاء المهلة؛ وهذا ليس خطأً بحد ذاته.
- يتم الإبلاغ عن أخطاء مستوى البروتوكول (CRC، رموز الاستثناءات، أخطاء IO) عبر `ModbusHook::on_error`.

نمط موصى به:

- اعتبار المهل العرضية أمرًا طبيعيًا في بيئات الاتصال التسلسلي غير المستقرة.
- استخدام عداد تراكمي في الـ hook الخاص بك؛ إذا تجاوزت الأخطاء المتتالية حدًا معينًا، قم بإطلاق إنذار.

---

## 6. تشغيل المثال

من جذر المستودع:

```bash
cargo run --package api_master -- /tmp/vcom1
```

في بيئة اختبار شبيهة بالإنتاج (مثل منصة اختبار خزان تخزين الهيدروجين)، تقوم عادةً بـ:

- استخدام Aoba CLI/TUI أو `examples/modbus_slave` لمحاكاة جانب الجهاز التابع.
- ثم تشغيل مثال `api_master` للتحقق من أن توصيلات Modbus ومنطق مستوى التطبيق يعملان كما هو متوقع.

---

## 7. الوضع اليدوي للجهاز الرئيسي (poll_once / عمليات الكتابة)

بالنسبة للسيناريوهات التي تحتاج فيها إلى تحكم دقيق في توقيت الاستقصاء (آلات الحالة، الاستراتيجيات التكيفية، أو عمليات الكتابة)، استخدم `build_master_manual()`:

```rust
use anyhow::Result;
use _main::api::modbus::{ModbusBuilder, RegisterMode};

fn main() -> Result<()> {
    let master = ModbusBuilder::new_master(1)
        .with_port("/dev/ttyUSB0")
        .with_baud_rate(9600)
        .with_timeout(5000)
        .build_master_manual()?;

    // استقصاء فردي يدوي
    let response = master.poll_once(RegisterMode::Holding, 0x00, 10)?;
    println!("Values: {:?}", response.values);

    // كتابة سجل حفظ واحد (fc 0x06)
    master.write_holding(0x00, 0x1234)?;

    // كتابة سجلات حفظ متعددة (fc 0x10)
    master.write_registers(0x00, &[0x1234, 0x5678, 0x9ABC])?;

    // كتابة ملفات (fc 0x0F)
    master.write_coils(0x00, &[true, false, true, true])?;

    Ok(())
}
```

### متى تستخدم الوضع اليدوي

| السيناريو | الوضع الموصى به |
|-----------|-----------------|
| المراقبة المستمرة / جمع البيانات | `build_master()` (تلقائي) |
| حلقات تحكم قراءة-تعديل-كتابة | `build_master_manual()` |
| آلة الحالة / الاستقصاء المدفوع بالأحداث | `build_master_manual()` |
| الاستقصاء التكيفي بناءً على زمن استجابة | `build_master_manual()` |
| التشخيص أو التكوين لمرة واحدة | `build_master_manual()` |

### تفاصيل عمليات الكتابة

- **`write_holding(address, value)`** — يكتب سجل حفظ واحدًا باستخدام كود الدالة 0x06. الأفضل لكتابة معاملات التكوين الفردية.
- **`write_registers(address, values)`** — يكتب سجلات حفظ متعددة متتالية باستخدام كود الدالة 0x10. الأفضل لكتابة الدُفعات من المعاملات.
- **`write_coils(address, values)`** — يكتب ملفات متعددة باستخدام كود الدالة 0x0F. يتضمن تبديل بايت تلقائي لكتابة 11 ملفًا (مطلوب من قبل بعض الأجهزة).
- جميع طرق الكتابة تحظر حتى يؤكد الجهاز التابع الاستلام أو يحدث خطأ.

---

## 8. الخطوات التالية

- لواجهات برمجة التطبيقات من جانب الجهاز التابع، راجع `examples/api_slave`.
- لاستخدام Modbus على مستوى CLI، راجع `docs/ar/CLI_MODBUS.md`.
- لتصدير البيانات عبر HTTP / MQTT / IPC، راجع مستندات `DATA_SOURCE_*.md` في هذا الدليل.
