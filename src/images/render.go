package main

import (
	"strings"
	"bufio"
	"net/http"
	"image"
	"log"
	"io"
	"net"
	"fmt"
	_ "image/png"
	_ "image/jpeg"
	_ "golang.org/x/image/webp"
)

var chars = []rune{' ', '░', '▒', '▓'}

type ascii_fn func(image.Image, int, int) string

func pix_to_bw(img image.Image, x, y int) string {
	var r, g, b uint32
	var lightness float64

	r, g, b, _ = img.At(x, y).RGBA()

	lightness = 0.2126 * float64(r) / float64(0xffff) + 0.7152 * (float64(g) / float64(0xffff)) + 0.0722 * (float64(b) / float64(0xffff))

	k := max(int32(lightness * 4) - 1, 0)
	return string(chars[k])
}

func pix_to_rgb(img image.Image, x, y int) string {
	var r, g, b uint32

	r, g, b, _ = img.At(x, y).RGBA()
	rf := float64(r) / float64(0xffff)
	gf := float64(g) / float64(0xffff)
	bf := float64(b) / float64(0xffff)

	rs := int(rf * 255)
	gs := int(gf * 255)
	bs := int(bf * 255)

	return fmt.Sprintf("\033[38;2;%d;%d;%dm█", rs, gs, bs)
}

func compress(img image.Image, converter ascii_fn) string {
	var ret string

	target_width := 100
	width := img.Bounds().Max.X - img.Bounds().Min.X

	height := img.Bounds().Max.Y - img.Bounds().Min.Y
	target_height := int(float64(height) / float64(width) / 2.0 * float64(target_width))

	xstride := width / target_width
	ystride := height / target_height

	for y := range(target_height) {
		for x := range 100 {
			ret += converter(img, x * xstride, y * ystride)
		}
		ret += "\033[0m\n"
	}

	return ret
}

func make_image(r io.Reader, converter *ascii_fn) (string, error) {
	reader := bufio.NewReader(r)
	line, err := reader.ReadString('\n')
	if err != nil {
		log.Printf("err: %w", err)
		return "fucky wucky\n", err
	}

	line = strings.TrimSpace(line)
	if line == "color" {
		*converter = pix_to_rgb
		return "Using RGB.\n", nil
	} else if line == "bw" {
		*converter = pix_to_bw
		return "Using BW.\n", nil
	}

	resp, err := http.Get(line)
	if err != nil {
		log.Printf("err: %w", err)
		return "other fucky wucky\n", err
	}

	img, _, err := image.Decode(resp.Body)
	if err != nil {
		log.Fatalf("%w", err)
		return "fucky wucky!\n", err
	}

	return compress(img, *converter), nil
}

func handleConn(conn net.Conn) {
	var converter ascii_fn
	converter = pix_to_rgb

	conn.Write([]byte("Welcome! Paste an image URL to view. Commands 'color' and 'bw' can be used to alter the output.\n"))

	for {
		img, err := make_image(conn, &converter)
		conn.Write([]byte(img))

		if err != nil {
			log.Printf("%w", err)
			break
		}
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
