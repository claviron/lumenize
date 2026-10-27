<?php

declare(strict_types=1);

// Exercise the actual codec without a native cache, application or server.
require $argv[1] ?? throw new LogicException('Supply a Composer autoloader');

use Lumenize\Cache\Store;

final class CacheWakeProbe
{
    public static int $wakes = 0;
    public string $value = 'example';

    public function __wakeup(): void { ++self::$wakes; }
}

final class CacheSerializationProbe
{
    public static int $reads = 0;
    public string $value = 'example';

    public function __serialize(): array { return ['value' => $this->value]; }
    public function __unserialize(array $data): void
    {
        ++self::$reads;
        $this->value = $data['value'];
    }
}

$check = static function (bool $condition, string $message): void {
    if (!$condition) { throw new LogicException($message); }
};
$encode = new ReflectionMethod(Store::class, 'encode');
$decode = new ReflectionMethod(Store::class, 'decode');
$marker = "\0claviron:object\0";
$strings = [
    $marker.serialize(new CacheWakeProbe()),
    $marker.serialize(new CacheSerializationProbe()),
    $marker,
    $marker.'not serialized',
    $marker.'b:0;',
    $marker.'N;',
    $marker.'a:1:{',
    $marker.$marker.'nested',
    $marker."binary\0\xff\";data",
    $marker.'çeviri',
];

set_error_handler(static function (int $severity, string $message, string $file, int $line): never {
    throw new ErrorException($message, 0, $severity, $file, $line);
});
try {
    foreach ($strings as $value) {
        $encoded = $encode->invoke(null, $value);
        $check($decode->invoke(null, $encoded) === $value, 'A marker-prefixed string did not round-trip unchanged');
        $check(CacheWakeProbe::$wakes === 0 && CacheSerializationProbe::$reads === 0, 'A plain string invoked an object deserialization hook');
        // An old reader already decodes this same marker with unserialize.
        $check(unserialize(substr($encoded, strlen($marker))) === $value, 'An escaped string is incompatible with existing readers');
        $check($encode->invoke(null, $decode->invoke(null, $encoded)) === $encoded, 'Reading and rewriting a string accumulated escaping');
    }

    foreach ([null, false, true, 0, -1, PHP_INT_MAX, 1.25, '', 'ordinary', 'b:0;', "binary\0\xff", ['value' => $strings[0], 'enabled' => true]] as $value) {
        $encoded = $encode->invoke(null, $value);
        $check($encoded === $value, 'An ordinary scalar or array stopped using its native representation');
        $check($decode->invoke(null, $encoded) === $value, 'A native cache value did not round-trip');
    }

    foreach ([(object) ['value' => 'example'], new CacheWakeProbe(), new CacheSerializationProbe()] as $object) {
        $legacy = $marker.serialize($object);
        $check($encode->invoke(null, $object) === $legacy, 'Object wire representation changed');
        $decoded = $decode->invoke(null, $legacy);
        $check($decoded == $object && $decoded !== $object, 'An existing serialized object did not round-trip');
    }
    $check(CacheWakeProbe::$wakes === 1 && CacheSerializationProbe::$reads === 1, 'Intentional object deserialization hooks were not preserved');
} finally {
    restore_error_handler();
}

echo "PASS: native cache values, marker-prefixed strings, object support and existing-reader compatibility\n";
