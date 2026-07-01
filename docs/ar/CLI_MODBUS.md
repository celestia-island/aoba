# ميزات واجهة سطر الأوامر لـ Modbus

يصف هذا المستند ميزات واجهة سطر الأوامر الجديدة لعمليات Modbus المضافة إلى مشروع aoba.

## الميزات

### 1. اكتشاف المنافذ وسردها

#### سرد جميع المنافذ

يوفر الأمر `--list-ports` الآن معلومات أكثر تفصيلاً عند استخدامه مع `--json`:

```bash
aoba --list-ports --json
```

يتضمن المُخرج:

- `path`: مسار المنفذ (مثال: COM1، /dev/ttyUSB0)
- `status`: "Free" أو "Occupied"
- `guid`: معرف GUID لجهاز Windows (إن توفر)
- `vid`: معرف بائع USB (إن توفر)
- `pid`: معرف منتج USB (إن توفر)
- `serial`: الرقم التسلسلي (إن توفر)

مثال على المُخرج:

```json
[
  {
    "path": "COM1",
    "status": "Free",
    "guid": "{...}",
    "vid": 1234,
    "pid": 5678
  }
]
```

#### التحقق من حالة إشغال منفذ واحد

يُستخدم الأمر `--check-port` للكشف عن ما إذا كان منفذ معين مشغولاً. وهذا مفيد لأتمتة النصوص البرمجية ومراقبة حالة المنافذ:

```bash
aoba --check-port COM3
```

**رموز الخروج:**

- `0` - المنفذ حر ومتاح
- `1` - المنفذ مشغول بواسطة برنامج آخر

**المُخرج النصي العادي:**

```
Port COM3 is free
```

أو

```
Port COM3 is occupied
```

**المُخرج بتنسيق JSON:**

```bash
aoba --check-port COM3 --json
```

مثال على المُخرج:

```json
{"port":"COM3","occupied":false,"status":"Free"}
```

أو

```json
{"port":"COM3","occupied":true,"status":"Occupied"}
```

**أمثلة على الاستخدام:**

الاستخدام في نصوص shell:

```bash
# مثال Bash
if aoba --check-port /dev/ttyUSB0; then
    echo "Port is free, ready to use"
    # قم بتنفيذ عملياتك
else
    echo "Port is occupied, please close the program using this port"
    exit 1
fi
```

```powershell
# مثال PowerShell
cargo run --package aoba -- --check-port COM3
if ($LASTEXITCODE -eq 0) {
    Write-Host "Port is free"
} else {
    Write-Host "Port is occupied"
}
```

### 2. أوضاع استماع الجهاز التابع

#### الوضع المؤقت

يستمع لطلب Modbus واحد، ويستجيب، ثم يخرج:

```bash
aoba --slave-listen /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --baud-rate 9600
```

يُخرج استجابة JSON واحدة ثم يخرج.

#### الوضع المستمر

يستمع للطلبات بشكل مستمر ويُخرج JSONL:

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --baud-rate 9600
```

يُخرج سطر JSON واحد لكل طلب تتم معالجته.

### 3. أوضاع توفير البيانات للجهاز الرئيسي

- الوضع المؤقت، يوفر البيانات مرة واحدة ثم يخرج:

```bash
aoba --master-provide /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

يقرأ سطرًا واحدًا من مصدر البيانات، ويرسله، ثم يخرج.

- الوضع المستمر، يوفر البيانات بشكل مستمر:

```bash
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

يقرأ الأسطر من مصدر البيانات ويرسلها بشكل مستمر.

### تنسيق مصدر البيانات

لأوضاع الجهاز الرئيسي، يجب أن يحتوي ملف مصدر البيانات على تنسيق JSONL:

```json
{"values": [10, 20, 30, 40, 50]}
{"values": [15, 25, 35, 45, 55]}
{"values": [20, 30, 40, 50, 60]}
```

يمثل كل سطر تحديثًا سيتم إرساله إلى الجهاز التابع.

#### استخدام الملفات كمصدر بيانات

```bash
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

#### استخدام الأنابيب المُسمّاة في Unix كمصدر بيانات

يمكن استخدام الأنابيب المُسمّاة في Unix (FIFOs) لتدفق البيانات في الوقت الفعلي:

```bash
# إنشاء أنبوب مُسمّى
mkfifo /tmp/modbus_input

# بدء الجهاز الرئيسي في طرفية واحدة
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source pipe:/tmp/modbus_input \
  --baud-rate 9600

# كتابة البيانات في طرفية أخرى
echo '{"values": [10, 20, 30, 40, 50]}' > /tmp/modbus_input
```

### وجهات المُخرج

لأوضاع الجهاز التابع، يمكنك تحديد وجهات المُخرج:

#### المُخرج إلى stdout (افتراضي)

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600
```

#### المُخرج إلى ملف (وضع الإلحاق)

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600 \
  --output file:/path/to/output.jsonl
```

#### المُخرج إلى أنبوب مُسمّى في Unix

```bash
# إنشاء أنبوب مُسمّى
mkfifo /tmp/modbus_output

# بدء الجهاز التابع في طرفية واحدة
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600 \
  --output pipe:/tmp/modbus_output

# قراءة البيانات في طرفية أخرى
cat /tmp/modbus_output
```

## وضع الخدمة (التشغيل المستمر)

تدعم واجهة سطر الأوامر التشغيل المستمر الشبيه بالخدمة من خلال **أوضاع الاستمرار**:

- **خدمة الجهاز التابع**: استخدم `--slave-listen-persist` للاستماع والاستجابة المستمرين
- **خدمة الجهاز الرئيسي**: استخدم `--master-provide-persist` لتوفير البيانات المستمر

تعمل هذه الأوضاع إلى أجل غير مسمى حتى يتم مقاطعتها (Ctrl+C) وتُخرج JSONL (كائن JSON واحد لكل سطر) لكل عملية. وهي مثالية لـ:

- تطبيقات المراقبة طويلة الأمد
- أنظمة تسجيل البيانات
- التكامل مع أدوات أخرى عبر الأنابيب أو الملفات
- اتصال العمليات الفرعية في TUI (عند الدمج مع `--ipc-channel`)

مثال على استخدام الخدمة:

```bash
# التشغيل كخدمة جهاز تابع مع تسجيل المُخرج إلى ملف
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-mode holding \
  --output file:/var/log/modbus-slave.jsonl

# التشغيل كخدمة جهاز رئيسي مع إدخال من أنبوب
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-mode holding \
  --data-source pipe:/tmp/modbus_data
```

**ملاحظة**: يستخدم وضع TUI أوضاع الاستمرار هذه داخليًا مع `--ipc-channel` للاتصال ثنائي الاتجاه مع العمليات الفرعية لـ CLI.

## المعاملات

| المعامل | الوصف | الافتراضي |
|-----------|-------------|---------|
| `--station-id` | معرف محطة Modbus (عنوان الجهاز التابع) | 1 |
| `--register-address` | عنوان بداية السجل | 0 |
| `--register-length` | عدد السجلات | 10 |
| `--register-mode` | نوع السجل: holding، input، coils، discrete | holding |
| `--data-source` | مصدر البيانات: `file:<path>` أو `pipe:<name>` | - |
| `--output` | وجهة المُخرج: `file:<path>` أو `pipe:<name>` (الافتراضي: stdout) | stdout |
| `--baud-rate` | معدل الباود للمنفذ التسلسلي | 9600 |
| `--debounce-seconds` | نافذة منع الارتداد لمُخرج JSON المكرر (ثوانٍ، عدد عشري) | 1.0 |
| `--ipc-channel` | معرف UUID لقناة IPC لاتصال TUI (للاستخدام الداخلي) | - |

## أوضاع السجلات

- `holding`: سجلات الحفظ (قراءة/كتابة)
- `input`: سجلات الإدخال (قراءة فقط)
- `coils`: الملفات (بتات قراءة/كتابة)
- `discrete`: المدخلات المنفصلة (بتات قراءة فقط)

## اختبارات التكامل

تتوفر اختبارات التكامل في `examples/cli_e2e/`. قم بتشغيلها باستخدام:

```bash
cd examples/cli_e2e
cargo run
```

### تشغيل الاختبارات في وضع التكرار

لاختبار الاستقرار وتصحيح الأخطاء، يمكنك تشغيل الاختبارات عدة مرات باستخدام وسيط سطر الأوامر `--loop-count`:

```bash
# تشغيل الاختبارات 5 مرات متتالية
cargo run --example cli_e2e -- --loop-count 5

# تشغيل الاختبارات 10 مرات للتحقق من تنظيف المنفذ واستقراره
cargo run --example cli_e2e -- --loop-count 10
```

هذا مفيد لـ:

- التحقق من تنظيف المنفذ بين تشغيلات الاختبار
- اختبار الاستقرار وقابلية التكرار
- تصحيح المشكلات المتقطعة
- ضمان عمل إعادة تعيين المنفذ الافتراضي لـ socat بشكل صحيح

تتحقق الاختبارات من:

- سرد المنافذ المُحسَّن مع الحالة
- وضع الاستماع المؤقت للجهاز التابع
- وضع الاستماع المستمر للجهاز التابع
- وضع توفير البيانات المؤقت للجهاز الرئيسي
- وضع توفير البيانات المستمر للجهاز الرئيسي
- اختبار الاتصال المستمر (ملف كمصدر بيانات وملف كمُخرج)
- اختبار الاتصال المستمر (أنبوب Unix كمصدر بيانات وأنبوب كمُخرج)

### اختبارات الاتصال المستمر

تتحقق اختبارات الاتصال المستمر من نقل البيانات طويل الأمد بين الجهاز الرئيسي والجهاز التابع:

1. **الملفات كمصدر بيانات ومُخرج**: يقرأ الجهاز الرئيسي البيانات من ملف ويرسلها، ويستقبلها الجهاز التابع ويُضيفها إلى ملف
2. **أنابيب Unix كمصدر بيانات ومُخرج**: يقرأ الجهاز الرئيسي البيانات في الوقت الفعلي من أنبوب مُسمّى، ويُخرجها الجهاز التابع إلى أنبوب مُسمّى
3. **توليد بيانات عشوائية**: يُولّد كل تشغيل اختبار بيانات عشوائية مختلفة لضمان موثوقية الاختبار

## التحسينات المستقبلية

- اختبارات اتصال Modbus في الوقت الفعلي مع منافذ تسلسلية افتراضية
- دعم إضافي لأنواع السجلات
