//usr/bin/env go run "$0" "$@"; exit "$?"
// contact: lays animations out one per row, frames side by side, on a grey
// ground so transparent edges read. Usage: contact.go out.png dir1 dir2 ...
// where each dir holds frame_000.png, frame_001.png, ...
package main

import (
	"fmt"
	"image"
	"image/color"
	"image/draw"
	"image/png"
	"os"
	"path/filepath"
	"sort"
)

func load(path string) image.Image {
	f, err := os.Open(path)
	if err != nil {
		panic(err)
	}
	defer f.Close()
	img, err := png.Decode(f)
	if err != nil {
		panic(err)
	}
	return img
}

func main() {
	if len(os.Args) < 3 {
		fmt.Fprintln(os.Stderr, "usage: contact.go out.png dir...")
		os.Exit(2)
	}
	var rows [][]image.Image
	cw, ch, cols := 0, 0, 0
	for _, dir := range os.Args[2:] {
		files, _ := filepath.Glob(filepath.Join(dir, "*.png"))
		sort.Strings(files)
		var row []image.Image
		for _, f := range files {
			img := load(f)
			b := img.Bounds()
			cw, ch = max(cw, b.Dx()), max(ch, b.Dy())
			row = append(row, img)
		}
		cols = max(cols, len(row))
		rows = append(rows, row)
	}
	const gap = 2
	out := image.NewRGBA(image.Rect(0, 0, cols*(cw+gap), len(rows)*(ch+gap)))
	draw.Draw(out, out.Bounds(), &image.Uniform{color.RGBA{40, 40, 48, 255}}, image.Point{}, draw.Src)
	for r, row := range rows {
		for c, img := range row {
			cell := image.Rect(c*(cw+gap), r*(ch+gap), c*(cw+gap)+cw, r*(ch+gap)+ch)
			draw.Draw(out, cell, &image.Uniform{color.RGBA{96, 96, 108, 255}}, image.Point{}, draw.Src)
			draw.Draw(out, cell, img, img.Bounds().Min, draw.Over)
		}
	}
	f, err := os.Create(os.Args[1])
	if err != nil {
		panic(err)
	}
	defer f.Close()
	if err := png.Encode(f, out); err != nil {
		panic(err)
	}
	fmt.Printf("%s: %d rows, %d cols, cell %dx%d\n", os.Args[1], len(rows), cols, cw, ch)
}
