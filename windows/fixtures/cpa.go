package main

import (
	"context"
	"flag"
	"fmt"
	"io"
	"net/http"
	"os"
	"os/signal"
	"path/filepath"
	"regexp"
	"time"
)

const revision = "first"

func main() {
	config := flag.String("config", "", "")
	flag.Parse()
	data, err := os.ReadFile(*config)
	if err != nil {
		panic(err)
	}
	port := regexp.MustCompile(`(?m)^  port: ([0-9]+)$`).FindSubmatch(data)
	if len(port) != 2 {
		panic("missing fixture port")
	}
	mux := http.NewServeMux()
	mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		fmt.Fprint(w, revision)
	})
	mux.HandleFunc("/v8/management/credentials", func(w http.ResponseWriter, r *http.Request) {
		io.WriteString(w, `{"files":[]}`)
	})
	mux.HandleFunc("/v8/management/config/access/api-keys", func(w http.ResponseWriter, r *http.Request) {
		io.WriteString(w, `[]`)
	})
	mux.HandleFunc("/v1/models", func(w http.ResponseWriter, r *http.Request) {
		io.WriteString(w, `{"data":[{"id":"managed-fixture","owned_by":"openai"}]}`)
	})
	server := &http.Server{Addr: "127.0.0.1:" + string(port[1]), Handler: mux}
	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt)
	defer cancel()
	done := make(chan struct{})
	go func() {
		defer close(done)
		<-ctx.Done()
		deadline, stop := context.WithTimeout(context.Background(), 2*time.Second)
		defer stop()
		if server.Shutdown(deadline) == nil {
			_ = os.WriteFile(filepath.Join(filepath.Dir(*config), "graceful-stop"), []byte(revision), 0600)
		}
	}()
	if err := server.ListenAndServe(); err != http.ErrServerClosed {
		panic(err)
	}
	<-done
}
