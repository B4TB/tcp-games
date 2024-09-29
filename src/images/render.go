package main

import (
	"strings"
	"bufio"
	"net/http"
	"image"
	"log"
	"io"
	"net"
	_ "image/png"
	_ "image/jpeg"
	_ "golang.org/x/image/webp"
)

var chars = []rune{' ', '░', '▒', '▓'}

func compress(img image.Image) string {
	var ret string
	var r, g, b, k uint32
	var lightness float64

	target_width := 100
	width := img.Bounds().Max.X - img.Bounds().Min.X

	height := img.Bounds().Max.Y - img.Bounds().Min.Y
	target_height := int(float64(height) / float64(width) / 2.0 * float64(target_width))

	xstride := width / target_width
	ystride := height / target_height

	for y := range(target_height) {
		for x := range 100 {
			r, g, b, _ = img.At(x * xstride, y * ystride).RGBA()

			lightness = 0.2126 * float64(r) / float64(0xffff) + 0.7152 * (float64(g) / float64(0xffff)) + 0.0722 * (float64(b) / float64(0xffffff))

			k = uint32(lightness * 4)

			ret += string(chars[k])
		}
		ret += "\n"
	}

	return ret
}

func make_image(r io.Reader) (string, error) {
	reader := bufio.NewReader(r)
	url, err := reader.ReadString('\n')
	if err != nil {
		log.Printf("err: %w", err)
		return "fucky wucky\n", err
	}

	resp, err := http.Get(strings.TrimSpace(url))
	if err != nil {
		log.Printf("err: %w", err)
		return "other fucky wucky\n", err
	}

	img, _, err := image.Decode(resp.Body)
	if err != nil {
		log.Fatalf("%w", err)
		return "fucky wucky!\n", err
	}

	return compress(img), nil
}

func handleConn(conn net.Conn) {
	for {
		img, err := make_image(conn)
		if err != nil {
			log.Printf("%w", err)
			break
		}

		conn.Write([]byte(img))
	}
}

func main() {
	log.Print("Binding: 0.0.0.0:5173")
	ln, err := net.Listen("tcp", ":5173")
	if err != nil {
		log.Fatalf("%w\n", err)
	}

	for {
		conn, err := ln.Accept()
		if err != nil {
			log.Printf("%w\n", err)
		}

		go handleConn(conn)
	}

	if err != nil {
		log.Fatalf("%w", err)
	}
}
