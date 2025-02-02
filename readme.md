[![crates.io](https://img.shields.io/crates/v/mdedit.svg)](https://crates.io/crates/mdedit)
[![Documentation](https://docs.rs/mdedit/badge.svg)](https://docs.rs/mdedit)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![License](https://img.shields.io/badge/license-APACHE-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
![](https://tokei.rs/b1/github/thscharler/mdedit)

# ratatui markdown editor

This application is built with [rat-salsa][refRatSalsa].

![image][refMDEditGif]


MDEDIT(1)            Markdown editing               MDEDIT(1)

# NAME

mdedit - markdown editing

# SYNOPSIS

mdedit [file ...]

# DESCRIPTION

A simple markdown editor with some syntax highlighting.

# USING MDEDIT

## Keyboard navigation

| Key                          | Description                      |
|------------------------------|----------------------------------|
| Esc                          | Jump to menu and back.           |
|                              |                                  |
| Tab / Backtab                | Navigate focus. Will not work to |
|                              | leave the editor, use Ctrl+W Tab |
|                              | there.                           |
|                              |                                  |
| F4                           | Jump to tree and back.           |
| F5                           | Jump to file list and back.      |
| F6                           | Hide/show file list.             |
| F2                           | Cheat sheet.                     |
| F1                           | This document.                   |

## File list

| Key                          | Description                      |
|------------------------------|----------------------------------|
| Enter                        | Open in current split.           |
| '+'                          | Open in new split.               |

If the file is already open it is selected instead.

## Ctrl-W - Window navigation

| Key                          | Description                      |
|------------------------------|----------------------------------|
| Ctrl-W Left/Right            | Jump between split windows.      |
|                              |                                  |
| Ctrl-W Tab/Backtab           | Change focus.                    |
|                              |                                  |
| Ctrl-W t                     | Jump to tabs. Use Left/Right     |
|                              | to navigate.                     |
|                              |                                  |
| Ctrl-W s                     | Jump to edit split. Use          |
|                              | Left/Right to resize and         |
|                              | Alt+Left/Alt+Right to navigate   |
|                              | splits.                          |
|                              |                                  |
| Ctrl-W f                     | Jump to file split. Use          |
|                              | Left/Right  to resize.           |
|                              |                                  |
| Ctrl-W c                     |                                  |
| Ctrl-W x                     |                                  |
| Ctrl+F4                      |                                  |
| Ctrl+e                       | Close the current window         |
|                              |                                  |
| Ctrl+Shift+F4                | Close all windows in the current |
| Ctrl+Shift+e                 | split.                           |
|                              |                                  |
| Ctrl-W d                     |                                  |
| Ctrl-W +                     | Split view                       |

## Files

| Key                          | Description                      |
|------------------------------|----------------------------------|
| Ctrl+O                       | Open file                        |
| Ctrl+N                       | New file                         |
| Ctrl+S                       | Save file. Auto-saved when the   |
|                              | terminal looses focus.           |

## Editing

| Key                          | Description                      |
|------------------------------|----------------------------------|
| Ctrl+C / Ctrl+X / Ctrl+V     | Clipboard                        |
|                              |                                  |
| Ctrl+Z / Ctrl+Shift+Z        | Undo / Redo                      |
|                              |                                  |
| Ctrl+D                       | Duplicate line.                  |
|                              |                                  |
| Ctrl+Y                       | Delete line.                     |
|                              |                                  |
| Ctrl+Backspace / Ctrl+Delete |                                  |
| Alt+Backspace / Alt+Delete   | Delete word.                     |
|                              |                                  |
| Tab / Backtab                | Indent/Dedent selection.         |
|                              | Insert tab otherwise.            |
|                              |                                  |
| Alt+1..6                     | Toggle header.                   |
| '_' / '*' / '~' + Selection  | Wrap the selected text with the  |
|                              | markup character.                |
| Alt+C                        | Add code quotes.                 |
| Alt+I                        | Add image link.                  |
| Alt+L                        | Add link.                        |
| Alt+K                        | Add reference link.              |
| Alt+R                        | Add reference.                   |
| Alt+F                        | Add footnote.                    |
|                              |                                  |
| any bracket + Selection      | Wrap the selected text with the  |
|                              | bracket.                         |

## Table

| Key                          | Description                      |
|------------------------------|----------------------------------|
| Any text                     | Will not maintain the table      |
|                              | delimiters, use Ctrl+F / Ctrl+G  |
|                              | to reformat when done.           |
|                              |                                  |
| Enter                        | Line break within the table.     |
|                              | It maintains the table structure |
|                              | and adds a new table row.        |
|                              | It can add a line-break inside   |
|                              | existing text too.               |
|                              |                                  |
| Tab / Backtab                | Navigate between cells.          |
|                              |                                  |
| F8                           | Format the table according to    |
|                              | the header widths. Overlong      |
|                              | cells are not cut or reformatted |
|                              | though.                          |
|                              |                                  |
| F7                           | Same as Ctrl+F but chooses the   |
|                              | max column width as width for    |
|                              | all columns.                     |

## Formatting

| Key                          | Description                      |
|------------------------------|----------------------------------|
| F8                           | Formats the item at the cursor   |
|                              | position, or everything          |
|                              | selected.                        |
|                              |                                  |
| F7                           | Alternate format.                |
|                              | Formats a table to with all      |
|                              | equal column widths.             |
|                              |                                  |
| Alt+1 .. Alt+6               | Flip header.                     |




[refMDEditGif]: https://github.com/thscharler/mdedit/blob/master/mdedit.gif?raw=true
