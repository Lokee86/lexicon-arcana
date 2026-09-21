package objectstore

import (
	"fmt"
	"unicode/utf8"
)

func (reader *binaryObjectReader) stringTableV2() ([]string, error) {
	count, err := reader.count("string table", maxBinaryStrings)
	if err != nil {
		return nil, err
	}
	if count == 0 {
		return nil, fmt.Errorf("Lexicon binary object has no empty string sentinel")
	}

	strings := make([]string, count)
	previous := []byte(nil)
	for index := range strings {
		prefix, err := reader.uvarint("string prefix length")
		if err != nil {
			return nil, err
		}
		if prefix > uint64(len(previous)) {
			return nil, fmt.Errorf(
				"Lexicon binary object string %d prefix %d exceeds previous length %d",
				index, prefix, len(previous),
			)
		}
		suffix, err := reader.bytes("string suffix", maxBinaryStringSize)
		if err != nil {
			return nil, err
		}
		if prefix > maxBinaryStringSize ||
			uint64(len(suffix)) > maxBinaryStringSize-prefix {
			return nil, fmt.Errorf(
				"Lexicon binary object string %d reconstructed length exceeds limit", index,
			)
		}

		value := make([]byte, int(prefix)+len(suffix))
		copy(value, previous[:int(prefix)])
		copy(value[int(prefix):], suffix)
		if !utf8.Valid(value) {
			return nil, fmt.Errorf("Lexicon binary object string %d is not valid UTF-8", index)
		}
		strings[index] = string(value)
		previous = value
	}
	if strings[0] != "" {
		return nil, fmt.Errorf("Lexicon binary object has invalid empty string sentinel")
	}
	return strings, nil
}
