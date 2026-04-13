# DISCLAIMER: ALL THIS CODE IS AI GENERATED
# Generates a marshal file exercising every type the reikland parser handles,
# including edge-case values for numeric types.
#
# Usage: ruby generate.rb
# Output: all_types.marshal in the same directory as this script

module TestModule; end

class TestObject
  def initialize(x, y)
    @x = x
    @y = y
  end
end

TestStruct = Struct.new(:x, :y)

class UserDefinedClass
  attr_reader :payload

  def initialize(payload = "default")
    @payload = payload
  end

  def _dump(_level)
    @payload
  end

  def self._load(str)
    new(str)
  end
end

class UserMarshalClass
  attr_reader :data

  def initialize(data = [1, 2, 3])
    @data = data
  end

  def marshal_dump
    @data
  end

  def marshal_load(obj)
    @data = obj
  end
end

class MyString < String; end

# A string used twice to trigger ObjectReference (@)
shared_string = "shared"

# A hash with a default value to trigger HashDefault (})
hash_with_default = Hash.new(42)
hash_with_default[:a] = 1
hash_with_default[:b] = 2

# An object extended with a module to trigger Extended (e)
extended_obj = TestObject.new(10, 20)
extended_obj.extend(TestModule)

data = [
  # -- Nil, True, False --
  nil,
  true,
  false,

  # -- Fixnum edge cases --
  # Zero (encoded as 0x00)
  0,
  # Single-byte positive (encoded as value + 5, range 1..122)
  1,
  122,
  # Single-byte negative (encoded as value - 5, range -123..-1)
  -1,
  -123,
  # 1-byte positive (0x01 prefix)
  123,
  255,
  # 1-byte negative (0xff prefix)
  -124,
  -256,
  # 2-byte positive (0x02 prefix)
  256,
  65535,
  # 2-byte negative (0xfe prefix)
  -257,
  -65536,
  # 3-byte positive (0x03 prefix)
  65536,
  16777215,
  # 3-byte negative (0xfd prefix)
  -65537,
  -16777216,
  # 4-byte (0x04/0xfc prefix)
  16777216,
  1073741823,    # max fixnum  (2^30 - 1)
  -16777217,
  -1073741824,   # min fixnum -(2^30)

  # -- Float edge cases --
  0.0,
  -0.0,
  1.5,
  -1.5,
  Float::INFINITY,
  -Float::INFINITY,
  Float::NAN,
  1.7976931348623157e+308,  # Float::MAX (DBL_MAX)
  2.2250738585072014e-308,  # Float::MIN (smallest normal)
  5.0e-324,                 # smallest subnormal (Float::MIN * Float::EPSILON)

  # -- Bignum edge cases --
  2**30,         # just above fixnum range (positive)
  -(2**30 + 1),  # just below fixnum range (negative)
  2**64,         # large positive
  -(2**64),      # large negative
  2**128,        # very large

  # -- Symbol and SymbolLink --
  # :test_symbol is new -> Symbol (:)
  # it will appear again inside hashes/objects -> SymbolLink (;)
  :test_symbol,

  # -- String (wrapped in Instance for encoding) --
  "hello world",
  "",
  "binary\x00data".b,  # ASCII-8BIT / binary encoding
  "\u{1F600}",          # multi-byte UTF-8

  # -- Regex (wrapped in Instance for encoding) --
  /simple/,
  /with flags/imx,

  # -- Array --
  [],
  [1, "two", :three, 4.0],

  # -- Hash --
  {},
  { a: 1, b: "two", c: :three },

  # -- HashDefault (}) --
  hash_with_default,

  # -- Object (o) --
  TestObject.new(42, "hello"),

  # -- Struct (S) --
  TestStruct.new(100, 200),

  # -- Extended (e) --
  extended_obj,

  # -- Class (c) --
  String,

  # -- Module (m) --
  Kernel,

  # -- UserDefined (u) --
  UserDefinedClass.new("custom_payload"),

  # -- UserMarshal (U) --
  UserMarshalClass.new([10, 20, 30]),

  # -- UserString (C) --
  MyString.new("subclassed string"),

  # -- ObjectReference (@) --
  # shared_string appears twice; second occurrence triggers @
  shared_string,
  shared_string,
]

output_path = File.join(__dir__, "all_types.marshal")
File.binwrite(output_path, Marshal.dump(data))

puts "Wrote #{File.size(output_path)} bytes to #{output_path}"
