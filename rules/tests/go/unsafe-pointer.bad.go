package mem

import "unsafe"

func Bytes(p *uint32) []byte {
	return (*[4]byte)(unsafe.Pointer(p))[:]
}
