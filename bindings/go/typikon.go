package typikon

/*
#cgo CFLAGS: -I../../include -I.
#cgo LDFLAGS: -L../../target/debug -Wl,-rpath,../../target/debug -ltypikon -Lnative/target/debug -Wl,-rpath,native/target/debug -ltypikon_go_native
#include "typikon.h"
*/
import "C"

import "fmt"

func ABIVersion() uint16 { return uint16(C.typikon_abi_version()) }

func NegotiateLayer(requested uint16, supported []uint16) (uint16, error) {
	var ptr *C.uint16_t
	if len(supported) > 0 {
		ptr = (*C.uint16_t)(&supported[0])
	}
	result := C.typikon_negotiate_layer(C.uint16_t(requested), ptr, C.size_t(len(supported)))
	if result < 0 {
		return 0, fmt.Errorf("unsupported Layer %d", requested)
	}
	return uint16(result), nil
}
