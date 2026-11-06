package wait

import (
	"context"
	"time"
)

func Pause(ctx context.Context) {
	select {
	case <-time.After(time.Second):
	case <-ctx.Done():
	}
}
