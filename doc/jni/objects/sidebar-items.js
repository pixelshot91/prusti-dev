window.SIDEBAR_ITEMS = {"enum":[["JValue","Rusty version of the JNI C `jvalue` enum. Used in Java method call arguments and returns."],["ReleaseMode","ReleaseMode"]],"struct":[["AutoArray","Auto-release wrapper for pointer-based generic arrays."],["AutoLocal","Auto-delete wrapper for local refs."],["AutoPrimitiveArray","Auto-release wrapper for pointer-based primitive arrays."],["GlobalRef","A global JVM reference. These are “pinned” by the garbage collector and are guaranteed to not get collected until released. Thus, this is allowed to outlive the `JNIEnv` that it came from and can be used in other threads."],["JByteBuffer","Lifetime’d representation of a `jobject` that is an instance of the ByteBuffer Java class. Just a `JObject` wrapped in a new class."],["JClass","Lifetime’d representation of a `jclass`. Just a `JObject` wrapped in a new class."],["JFieldID","Wrapper around `sys::jfieldid` that adds a lifetime. This prevents it from outliving the context in which it was acquired and getting GC’d out from under us. It matches C’s representation of the raw pointer, so it can be used in any of the extern function argument positions that would take a `jfieldid`."],["JList","Wrapper for JObjects that implement `java/util/List`. Provides methods to get, add, and remove elements."],["JListIter","An iterator over the keys and values in a map."],["JMap","Wrapper for JObjects that implement `java/util/Map`. Provides methods to get and set entries and a way to iterate over key/value pairs."],["JMapIter","An iterator over the keys and values in a map."],["JMethodID","Wrapper around `sys::jmethodid` that adds a lifetime. This prevents it from outliving the context in which it was acquired and getting GC’d out from under us. It matches C’s representation of the raw pointer, so it can be used in any of the extern function argument positions that would take a `jmethodid`."],["JObject","Wrapper around `sys::jobject` that adds a lifetime. This prevents it from outliving the context in which it was acquired and getting GC’d out from under us. It matches C’s representation of the raw pointer, so it can be used in any of the extern function argument positions that would take a `jobject`."],["JStaticFieldID","Wrapper around `sys::jstaticfieldid` that adds a lifetime. This prevents it from outliving the context in which it was acquired and getting GC’d out from under us. It matches C’s representation of the raw pointer, so it can be used in any of the extern function argument positions that would take a `jstaticfieldid`."],["JStaticMethodID","Wrapper around `sys::jmethodid` that adds a lifetime. This prevents it from outliving the context in which it was acquired and getting GC’d out from under us. It matches C’s representation of the raw pointer, so it can be used in any of the extern function argument positions that would take a `jmethodid`. This represents static methods only since they require a different set of JNI signatures."],["JString","Lifetime’d representation of a `jstring`. Just a `JObject` wrapped in a new class."],["JThrowable","Lifetime’d representation of a `jthrowable`. Just a `JObject` wrapped in a new class."]],"trait":[["TypeArray","Trait to define type array access/release"]]};