// Go reference for evidence/json/json.tarn (same hand-written parser).
package main

import (
	"errors"
	"fmt"
)

var errEnd = errors.New("unexpected end")

type parser struct {
	text []byte
	pos  int
	sum  int64
}

func (p *parser) peek() (byte, error) {
	if p.pos >= len(p.text) {
		return 0, errEnd
	}
	return p.text[p.pos], nil
}

func (p *parser) skipSpace() {
	for p.pos < len(p.text) && (p.text[p.pos] == ' ' || p.text[p.pos] == '\n') {
		p.pos++
	}
}

func (p *parser) expect(b byte) error {
	found, err := p.peek()
	if err != nil {
		return err
	}
	if found != b {
		return fmt.Errorf("unexpected %q", found)
	}
	p.pos++
	return nil
}

func (p *parser) literal(word string) error {
	for i := 0; i < len(word); i++ {
		if err := p.expect(word[i]); err != nil {
			return err
		}
	}
	return nil
}

func (p *parser) str() error {
	if err := p.expect('"'); err != nil {
		return err
	}
	for {
		b, err := p.peek()
		if err != nil {
			return err
		}
		p.pos++
		if b == '"' {
			return nil
		}
	}
}

func (p *parser) number() error {
	negative := false
	if b, err := p.peek(); err != nil {
		return err
	} else if b == '-' {
		negative = true
		p.pos++
	}
	var parsed int64
	digits := 0
	for p.pos < len(p.text) && p.text[p.pos] >= '0' && p.text[p.pos] <= '9' {
		parsed = parsed*10 + int64(p.text[p.pos]-'0')
		p.pos++
		digits++
	}
	if digits == 0 {
		return fmt.Errorf("expected digit")
	}
	if negative {
		parsed = -parsed
	}
	p.sum += parsed
	return nil
}

func (p *parser) value() error {
	p.skipSpace()
	b, err := p.peek()
	if err != nil {
		return err
	}
	switch b {
	case '{':
		return p.object()
	case '[':
		return p.array()
	case '"':
		return p.str()
	case 't':
		return p.literal("true")
	case 'f':
		return p.literal("false")
	case 'n':
		return p.literal("null")
	}
	return p.number()
}

func (p *parser) array() error {
	if err := p.expect('['); err != nil {
		return err
	}
	p.skipSpace()
	if b, err := p.peek(); err != nil {
		return err
	} else if b == ']' {
		p.pos++
		return nil
	}
	for {
		if err := p.value(); err != nil {
			return err
		}
		p.skipSpace()
		b, err := p.peek()
		if err != nil {
			return err
		}
		if b == ']' {
			p.pos++
			return nil
		}
		if err := p.expect(','); err != nil {
			return err
		}
	}
}

func (p *parser) object() error {
	if err := p.expect('{'); err != nil {
		return err
	}
	p.skipSpace()
	if b, err := p.peek(); err != nil {
		return err
	} else if b == '}' {
		p.pos++
		return nil
	}
	for {
		p.skipSpace()
		if err := p.str(); err != nil {
			return err
		}
		p.skipSpace()
		if err := p.expect(':'); err != nil {
			return err
		}
		if err := p.value(); err != nil {
			return err
		}
		p.skipSpace()
		b, err := p.peek()
		if err != nil {
			return err
		}
		if b == '}' {
			p.pos++
			return nil
		}
		if err := p.expect(','); err != nil {
			return err
		}
	}
}

func main() {
	text := []byte(`{"name": "tarn", "values": [1, 20, 300, {"deep": [4000, -5]}], "ok": true, "none": null, "pi": 3}`)
	var total int64
	for round := 0; round < 200000; round++ {
		p := parser{text: text}
		if err := p.value(); err != nil {
			panic(err)
		}
		total += p.sum
	}
	fmt.Println(total)
}
