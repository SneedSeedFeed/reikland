# Generates a marshal file designed to integration-test all the
# deserializer wrapper types in reikland::deserializer_types.
#
# The root is a Hash with symbol keys so it can be deserialized
# as a Rust struct (serde maps symbol keys to struct fields).
#
# Usage: ruby generate_wrapper_types.rb
# Output: wrapper_types.marshal in the same directory as this script

class Animal
  def initialize(name, legs)
    @name = name
    @legs = legs
  end
end

Pair = Struct.new(:left, :right)

# Hash with a default value
hash_with_default = Hash.new(99)
hash_with_default[:x] = 10
hash_with_default[:y] = 20
hash_with_default[:z] = 30

# Mixed-key hash (integer AND symbol keys pointing at data)
# This is the pattern DualKeyMap / DualKeyVec etc. are designed for.
# Contiguous 0-based int keys so DualKeyVecSparse works too.
mixed_hash = {
  0 => "zero",
  :alpha => "a",
  1 => "one",
  :beta  => "b",
  2 => "two",
  :gamma => "c",
}

# Mixed-key hash with gaps in int keys for DualKeyVecSparseHoley
sparse_hash = {
  0 => "first",
  :x => "ignored_x",
  5 => "sixth",
  :y => "ignored_y",
  2 => "third",
}

# Mixed-key hash keyed by integer for DualKeyMapInt
int_keyed_hash = {
  10 => "ten",
  :skip_a => "ignored",
  20 => "twenty",
  :skip_b => "ignored",
  30 => "thirty",
}

data = {
  # -- bare values (no Instance wrapper) for Transparent passthrough --
  bare_int: 42,
  bare_symbol: :my_symbol,

  # -- ivar-wrapped strings for Transparent / TransparentOpt / Ivar / WithEncoding --
  utf8_string: "hello world",
  ascii_string: "hello".encode("US-ASCII"),
  sjis_string: "\x82\xB1\x82\xF1\x82\xC9\x82\xBF\x82\xCD".force_encoding("Shift_JIS"),

  # -- regex for RbRegex --
  regex_plain: /hello/,
  regex_flags: /world/imx,

  # -- object for RbObject --
  animal: Animal.new("cat", 4),

  # -- struct for RbStruct --
  pair: Pair.new(100, 200),

  # -- hash with default for RbHashDefault --
  hash_default: hash_with_default,

  # -- mixed-key hashes for DualKey* types --
  mixed_hash: mixed_hash,
  sparse_hash: sparse_hash,
  int_keyed_hash: int_keyed_hash,
}

output_path = File.join(__dir__, "wrapper_types.marshal")
File.binwrite(output_path, Marshal.dump(data))

puts "Wrote #{File.size(output_path)} bytes to #{output_path}"
