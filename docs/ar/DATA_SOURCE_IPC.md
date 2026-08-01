# اتصال IPC (مصدر بيانات مخصص)

## بداية سريعة — تشغيل مستقبل CLI بسيط

بالنسبة لوضع مصدر البيانات `ipc:<path>` يقرأ CLI أسطر JSON من أنبوب مُسمّى (FIFO) أو ملف عادي. لبدء مستقبل CLI بسيط يقرأ من FIFO، قم بما يلي:

```bash
# إنشاء FIFO (مرة واحدة)
mkfifo /tmp/aoba_ipc.pipe

# بدء مستقبل CLI (سيقرأ الأسطر من مسار FIFO)
cargo run --bin aoba -- --master-provide-persist /tmp/vcom1 --data-source ipc:/tmp/aoba_ipc.pipe \
  --register-mode holding --register-address 0 --register-length 10

# ثم، من shell آخر، اكتب سطر JSON إلى الأنبوب:
echo '{"source":"ipc","type":"downlink","body":{"command":"ping"}}' > /tmp/aoba_ipc.pipe
```

ملاحظة: يستخدم المستودع أيضًا مآخذ نطاق Unix / الأنابيب المُسمّاة لاتصالات IPC الأخرى (TUI↔CLI). يتوقع وضع مصدر البيانات `ipc:<path>` تحديدًا مسار FIFO/ملف يمكن لـ CLI فتحه وقراءته سطرًا بسطر.

## نظرة عامة

يصف هذا المستند كيفية قبول التطبيق للبيانات المخصصة عبر IPC (الاتصال بين العمليات). في تصميم المستودع/التطبيق يعمل التطبيق كمستمع IPC (خادم)؛ يجب أن تعمل تكاملات الطرف الثالث أو البرامج المساعدة كعميل وترسل رسائل JSON إلى مقبس التطبيق. فيما يلي أمثلة من جانب العميل فقط (Rust/Python/Node) توضح كيفية الاتصال وإرسال رسالة.

## متى تستخدم IPC

- التكاملات المحلية حيث يكون عبء الشبكة غير ضروري
- اتصال سريع ومنخفض زمن الانتقال بين العمليات على نفس المضيف
- أدوات الاختبار وإعدادات E2E التي تنشئ عمليات مساعدة

## شكل الرسالة (موصى به)

استخدم JSON لقابلية النقل. مثال على رسالة:

```json
{
  "source": "ipc",
  "timestamp": "2025-11-15T12:34:56Z",
  "port": "/tmp/vcom1",
  "type": "downlink",
  "body": { "command": "write_register", "registers": [{"address":0, "value":"1234"}] }
}
```

## مقبس نطاق Unix: مثال Rust (باستخدام `interprocess`)

أضف التبعية في `Cargo.toml`:

```toml
[dependencies]
interprocess = "*"
```

يستمع التطبيق على مقبس نطاق Unix (على سبيل المثال `/tmp/aoba_ipc.sock`). يُظهر مثال Rust التالي كيف يمكن للعميل الاتصال بذلك المقبس وإرسال رسالة JSON واحدة.

العميل (اتصال وإرسال):

```rust
use std::io::{Read, Write};
use interprocess::local_socket::LocalSocketStream;

fn main() -> std::io::Result<()> {
    let mut stream = LocalSocketStream::connect("/tmp/aoba_ipc.sock")?;
    let msg = r#"{"source":"ipc","type":"downlink","body":{"command":"ping"}}"#;
    stream.write_all(msg.as_bytes())?;
    let mut resp = String::new();
    stream.read_to_string(&mut resp)?;
    println!("Response: {}", resp);
    Ok(())
}
```

ملاحظات:

- من المتوقع أن يقوم التطبيق بإنشاء وربط المقبس (المستمع). يجب ألا تحاول برامج العميل ربط نفس المسار — فهي تتصل فقط.
- إذا كنت تتحكم في كلا الجانبين للاختبارات، يمكنك تشغيل مستمع صغير محليًا؛ للإنتاج يوفر التطبيق مسار المقبس.
- على Windows استخدم Named Pipes (مسار مثل `\\.\pipe\aoba_ipc`) أو استخدم واجهات برمجة `interprocess` متعددة المنصات.

## مثال Python (AF_UNIX)

يقوم التطبيق بإنشاء وربط مقبس نطاق Unix؛ يُظهر مقتطف Python التالي كيف يتصل العميل ويرسل رسالة JSON إلى مسار مقبس التطبيق.

العميل:

```python
import socket
PATH = '/tmp/aoba_ipc.sock'
cli = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
cli.connect(PATH)
cli.sendall(b'{"source":"ipc","type":"downlink","body":{"command":"ping"}}')
resp = cli.recv(65536)
print('Response:', resp)
cli.close()
```

## مثال Node.js (ES6) — مقبس نطاق UNIX

يستمع التطبيق على مسار المقبس؛ يُظهر مقتطف Node.js التالي عميلاً يتصل ويرسل رسالة JSON.

العميل:

```javascript
import net from 'net';

const PATH = '/tmp/aoba_ipc.sock';
const client = net.createConnection({ path: PATH }, () => {
  client.write(JSON.stringify({ source: 'ipc', type: 'downlink', body: { command: 'ping' } }));
});

client.on('data', (data) => {
  console.log('Response:', data.toString());
  client.end();
});
```

## ملاحظات متعددة المنصات

- على Windows استخدم Named Pipes (`\\.\pipe\<name>`). لدى كل من Node و Python مكتبات للعمل مع الأنابيب المُسمّاة؛ يمكن لـ Rust استخدام `interprocess` للأنابيب متعددة المنصات.
- تأكد من أن صلاحيات ملف المقبس تسمح للعمليات بالاتصال.

إذا أردت، يمكنني توفير أداة اختبار صغيرة تنشئ الخادم والعميل وتوضح تبادل JSON الشامل ذهابًا وإيابًا بلغتك المفضلة.
