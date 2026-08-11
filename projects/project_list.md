The Big Book of Small Python Projects: Practice Projects Directory
This directory compiles all 81 easy practice programs from Al Sweigart's The Big Book of Small Python Projects. Designed for beginners and experienced coders alike, these self-contained, text-based projects are typically under 256 lines of code. They provide excellent exercise material for testing compiler or interpreter compliance, validating syntax parsing, and checking standard library implementations for any newly developed programming language.

Project Index & Descriptions
1. Bagels
A deductive logic game where you must guess a secret three-digit number based on clues. The program outputs "Pico" when a correct digit is guessed in the wrong place, "Fermi" when a correct digit is in the correct place, and "Bagels" when no digits are correct. Players have 10 tries to guess the secret number.

2. Birthday Paradox
Determine the probability that two people share the same birthday in groups of different sizes using a Monte Carlo simulation. This program conducts 100,000 randomized simulation trials to empirically demonstrate how group sizes affect probability.

3. Bitmap Message
Displays a customized text message arranged in a 2D pattern specified by a multiline string bitmap image (defaulting to a world map). Any non-space characters in the bitmap are replaced by repeating characters from the user's message.

4. Blackjack
A terminal implementation of the classic card game also known as 21, played against an AI dealer. The program draws cards as text graphics using Unicode suit symbols (hearts, diamonds, spades, clubs) and handles standard rules like hitting, standing, and doubling down.

5. Bouncing DVD Logo
A colorful screensaver animation that simulates a diagonally traveling DVD logo bouncing off the edges of the terminal window. It uses Cartesian coordinates, changes text color on each bounce, and tracks the total number of corner bounces.

6. Caesar Cipher
An ancient encryption algorithm used by Julius Caesar. It shifts uppercase letters over by a user-specified key (0 to 25) in the alphabet, wrapping around at 'Z'. Decryption shifts letters in the opposite direction.

7. Caesar Hacker
A brute-force cryptanalysis program that attempts to decrypt a Caesar-encrypted message without knowing the key. It attempts all 26 possible keys and displays the decrypted results for manual inspection.

8. Calendar Maker
Generates printable text files of monthly calendars for any year and month specified by the user. It leverages Python's datetime module and timedelta data type to calculate leap years and format the grid cleanly.

9. Carrot in a Box
A simple and silly two-player ASCII bluffing game. One player has a box containing a carrot, looks inside, and tells the other player whether they have it. The second player then decides whether to swap boxes, relying on screen clearing newlines for secrecy.

10. Cho-Han
A traditional Japanese gambling dice game played in feudal houses. Two six-sided dice are rolled in a cup, and the player guesses whether the sum is even (cho) or odd (han), with a 10% fee collected by the house on winning pots.

11. Clickbait Headline Generator
A humorous generator that programmatically creates thousands of clickbait-style headlines in seconds. It pulls random words from lists of states, nouns, places, pronouns, and timeframes and inserts them into Mad Libs-style templates.

12. Collatz Sequence
Generates numbers in the Collatz sequence (the simplest impossible conjecture in math) given a starting integer n. If n is even, the next number is n/2; if odd, it is 3n + 1. The program runs until the sequence terminates at 1.

13. Conway’s Game of Life
A 2D cellular automata simulation invented by mathematician John Conway. Cells on a grid live, die, or reproduce step-by-step according to simple rules based on their neighbors, creating complex and beautiful emergent behavior.

14. Countdown
An animated digital timer that counts down to zero from a specified number of seconds. It uses a seven-segment display module to render calculator-like digits and features an asterisks-based flashing colon separator.

15. Deep Cave
A scrolling terminal animation of a deep cave descending endlessly into the earth. It tracks the left wall width and gap width, randomly shifting them to create a moving subterranean visual.

16. Diamonds
A drawing algorithm that prints ASCII-art diamonds of various sizes. It contains separate functions for producing outline diamonds and filled-in diamonds, demonstrating pattern recognition and coordinate spacing.

17. Dice Math
A visual arithmetic quiz where the program rolls two to six dice and renders their faces at random, non-overlapping positions on a text canvas. Players must sum the displayed pips within a 30-second timeframe.

18. Dice Roller
A parsing tool for reading Dungeons & Dragons tabletop notation (such as "3d6" or "1d10+2") to generate random rolls. It handles basic multipliers and adjustments, allowing rolls of non-physical dice (e.g., 38-sided dice).

19. Digital Clock
Displays a real-time digital clock showing the current system time using a calculator-like seven-segment font. The display splits hours, minutes, and seconds and updates precisely every second.

20. Digital Stream
A terminal screensaver that mimics the scrolling binary "rain" visualization from The Matrix. Streams of 1s and 0s cascade vertically at random column intervals and adjustable animation speeds.

21. DNA Visualization
A short scrolling animation that draws an endless ASCII-art double helix. It generates random nucleotide pairs (guanine-cytosine and adenine-thymine) and formats them into a cycling list of horizontal templates.

22. Ducklings
An endless scrolling screensaver that generates a variety of cute ASCII-art ducklings. Using a class-based structure, the program mixes and matches directions, body sizes, eyes, mouths, and wing positions for 96 distinct variations.

23. Etching Drawer
An art program inspired by the classic Etch A Sketch toy. Players move a drawing cursor around the canvas with the WASD keys to trace continuous lines of box-drawing characters and can save their completed designs to a text file.

24. Factor Finder
Finds all the multiplicative factors of any positive whole number. It uses the modulo operator to test for remainders and optimizes calculations by searching up to the square root of the number.

25. Fast Draw
Tests reaction speed by prompting the player to press ENTER as soon as "DRAW!" appears on the screen. The program tracks elapsed time and penalizes players who trigger an early draw before the cue appears.

26. Fibonacci
Generates numbers in the famous Fibonacci sequence starting from 0 and 1, where each subsequent number is the sum of the two preceding numbers. It includes a performance warning for terms above 10,000.

27. Fish Tank
A calm and colorful software aquarium containing animated ASCII-art fish, kelp plants, and rising bubbles. It optimizes terminal performance by redrawing only the parts of the screen that change, reducing flickering.

28. Flooder
A grid-fill puzzle game where players attempt to fill an entire board with a single color/shape from the top-left tile. It uses a recursive flood-fill algorithm and includes a dedicated shape-based mode for colorblind players.

29. Forest Fire Sim
A cellular automata simulation that models the spread of wildfires. Trees grow on empty tiles, lightning strikes trees at random, and fire spreads dynamically to adjacent forest cells, demonstrating emergent behavior.

30. Four in a Row
A two-player board game (similar to Connect Four) where players take turns dropping tiles (X and O) into a grid. The goal is to connect four tiles horizontally, vertically, or diagonally while blocking the opponent.

31. Guess the Number
A classic number guessing game. The computer picks a pseudorandom number from 1 to 100, and the player has 10 attempts to find it, guided by "too high" or "too low" feedback.

32. Gullible
A short, humorous joke program that keeps a gullible person busy for hours. It asks a repeating yes/no question and loops endlessly on "yes" while exiting on "no".

33. Hacking Minigame
A hacking puzzle inspired by Fallout 3. Players attempt to deduce a secret seven-letter password from a grid of words surrounded by memory addresses and complex garbage characters, receiving feedback on matching letters.

34. Hangman and Guillotine
The classic word guessing game where players deduce a secret animal word letter-by-letter. Missed letters prompt the drawing of a gallows or, alternatively, a French guillotine.

35. Hex Grid
A brief, tiled art program that programmatically prints a repeating hexagonal grid resembling chicken wire. It illustrates how simple loops can quickly generate complex geometric text patterns.

36. Hourglass
An animation of an hourglass featuring a basic physics engine that simulates falling sand. Grains fall straight down, slide left or right on obstructions, stack in the bottom half, and reset when the hourglass rotates.

37. Hungry Robots
A grid-based game where the player must navigate a maze of walls while avoiding killer robots that move directly toward them. The player must trick the robots into crashing into each other or existing crash sites.

38. J’Accuse!
A detective mystery game where players must find Zophie the missing cat in five minutes. Players travel by taxi to interview suspects who either always lie or always tell the truth to deduce the culprit, location, and item.

39. Langton’s Ant
A cellular automata simulation invented by Chris Langton. An "ant" moves across a grid of black and white tiles, changing tile colors and rotating 90 degrees left or right depending on the color it lands on.

40. Leetspeak
A text translation utility that automatically converts plain English messages into l33t5p34]<. It utilizes a dictionary of letter mappings and randomly chooses substitutions to keep output varied.

41. Lucky Stars
A press-your-luck multiplayer party dice game inspired by Zombie Dice. Players pull dice of varying risk levels (Gold, Silver, Bronze) from a cup and roll to collect stars while avoiding three game-over skulls.

42. Magic Fortune Ball
A program modeled on the classic Magic 8 Ball toy. It accepts user questions and slowly displays spooky, stylized, uppercase yes/no predictions with lowercase 'i's and interspersed spacing.

43. Mancala
An implementation of the ancient, 2,000-year-old seed-sowing board game. Two players sow seeds counterclockwise across pockets, capturing seeds from opponent pits and earning free turns for landing in their store.

44. Maze Runner 2D
A top-down bird's-eye view maze game where players move an '@' character through maze text files using the WASD keys to reach an exit marker ('X').

45. Maze Runner 3D
A first-person 3D perspective maze runner. It generates a three-dimensional view from inside a maze by programmatically pasting "closed wall" ASCII art segments on top of an "all open" horizontal view.

46. Million Dice Roll Statistics Simulator
Empirically calculates the mathematical probabilities of dice rolls by rolling N six-sided dice one million times. It crunches the rolls in real-time and outputs percentage frequencies for each total sum.

47. Mondrian Art Generator
Programmatically generates geometric artwork in the minimalist style of Piet Mondrian. It creates a grid of black lines, deletes random segments to form larger rectangles, and fills them with red, yellow, blue, or black.

48. Monty Hall Problem
An educational simulation of the classic game show probability paradox. Players select one of three doors, the host reveals a goat behind an unopened door, and the program tracks the success rates of swapping versus staying.

49. Multiplication Table
Generates and prints a cleanly formatted multiplication grid from 0 × 0 up to 12 × 12. It serves as a straightforward demonstration of nested loops and right-justified string spacing.

50. Ninety-Nine Bottles
Programmatically prints the full lyrics to the repetitive folk song "Ninety-Nine Bottles of Milk on the Wall", using a loop for stanzas 99 to 2 and a separate block for the singular final stanza.

51. niNety-nniinE BoOttels
An extended, silly version of the "Ninety-Nine Bottles" program. With each verse, the script introduces random typos, casing swaps, letter doublings, and transpositions to programmatically mutate and distort the lyrics.

52. Numeral Systems Counters
Displays equivalent numbers across decimal (base-10), hexadecimal (base-16), and binary (base-2) numeral systems. It prints columns of equivalent counts from a user-specified starting value and quantity.

53. Periodic Table of the Elements
An interactive chemistry database program. It parses a CSV file (periodictable.csv) containing chemical information and allows users to query elements by atomic number or symbol to view atomic details.

54. Pig Latin
An English-to-Pig Latin translator. It moves initial consonant clusters to the end of words and appends "ay", or appends "yay" to vowel-starting words, while maintaining titlecase and uppercase formatting.

55. Powerball Lottery
Simulates buying up to one million lottery tickets to experience the thrill of losing without spending money. It highlights the actual odds (1 in 292,201,338) by demonstrating consistent losses against the winning numbers.

56. Prime Numbers
Uses brute-force calculations to find and output prime numbers sequentially. It tests odd numbers for divisibility up to their square root, allowing users to start search parameters from high numbers.

57. Progress Bar
A download task simulation that features a single-line animated progress bar. It demonstrates terminal manipulation by using the backspace escape character () to overwrite the bar on a single line.

58. Rainbow
A colorful terminal animation that draws a rainbow pattern zigzagging back and forth across the screen. It achieves a wave-like visual effect by incrementing and decrementing whitespace margins on a scrolling loop.

59. Rock Paper Scissors
The classic hand game played against the computer. It handles move selection, registers wins, losses, and ties, and builds suspense by counting down "1... 2... 3..." with short pauses.

60. Rock Paper Scissors (Always-Win Version)
A rigged version of Rock Paper Scissors designed to play a joke on friends. It omits loss/tie tracking and forces the computer to always select the move that loses to the player's choice.

61. ROT13 Cipher
A simple alphabetical shift cipher that encrypts and decrypts messages by rotating letters by exactly 13 places. The program preserves case-sensitivity and ignores numbers or punctuation.

62. Rotating Cube
A rotating 3D wireframe cube rendered in text blocks. It uses trigonometric rotation matrices to rotate points around the X, Y, and Z axes and employs Bresenham's line algorithm to draw the connecting edges.

63. Royal Game of Ur
An interactive implementation of the 5,000-year-old race game from Mesopotamia. Two players flip four pyramid dice to race seven tokens across a board, featuring safe flower spaces and capture mechanics.

64. Seven-Segment Display Module
A module designed to convert strings of numbers into calculator-style seven-segment text graphics. It supports negative signs and decimal points and is intended to be imported by other scripts.

65. Shining Carpet
Generates a repeating, interlocking hexagonal text tessellation modeling the iconic carpet design of the Overlook Hotel in Stanley Kubrick’s horror film The Shining.

66. Simple Substitution Cipher
An encryption scheme that maps each of the 26 letters of the alphabet to a randomized, unique substitution letter. It checks key validity to ensure all letters are represented once.

67. Sine Message
Scrolls a user-entered text message vertically in a wavy sine pattern. It evaluates a step counter in the math.sin function to calculate horizontal whitespace padding for each printed line.

68. Sliding Tile Puzzle
The classic 15-puzzle played on a 4 × 4 board. Players slide numbered tiles into a single blank space using the WASD keys, attempting to restore them to alphabetical and numerical order.

69. Snail Race
A slow-paced, humorous racing simulator where up to eight customizable snails compete. Snails crawl forward at random intervals, leaving behind a period-based slime trail toward the finish line.

70. Soroban Japanese Abacus
A graphical computer simulation of a traditional counting frame. Users can increment and decrement place columns using keyboard shortcuts or enter decimal numbers to slide beads accordingly.

71. Sound Mimic
An audio-based Simon-like pattern matching game. It uses a third-party audio player to play four distinct wav sounds corresponding to keys A, S, D, and F, prompting the player to repeat an increasingly long pattern.

72. sPoNgEcAsE
Converts user-entered text into sarcasm-themed "spongecase" by randomly swapping letter capitalization. It has a 90% chance to toggle case on each alphabetical character.

73. Sudoku Puzzle
A terminal implementation of the classic 9 × 9 deduction game. It loads puzzle boards from a text file, allows moves via column-row coordinates, features undo capabilities, and checks for solved columns, rows, and boxes.

74. Text-to-Speech Talker
An easy-to-use speech synthesis script that vocalizes any text entered by the user. It leverages a third-party wrapper module to coordinate with the operating system's native TTS engines.

75. Three-Card Monte
A card guessing game where the player tracks the Queen of Hearts through a series of randomized swaps. It features an optional cheat that swaps the Queen away from whichever card the player selects.

76. Tic-Tac-Toe
The classic 3 × 3 grid game of Xs and Os. It maps moves to a keypad grid (1-9), validates spaces, switches player turns, and checks for three-in-a-row or boards filled by a tie.

77. Tower of Hanoi
A stack-moving puzzle where players move a tower of five disks across three poles. Disks are represented by list elements, and the program enforces rules preventing larger disks from stacking on smaller ones.

78. Trick Questions
A quiz containing 54 deceptively simple, misleading questions. It uses free-form text input and evaluates answers against lists of correct keywords, demonstrating basic text-matching techniques.

79. Twenty Forty-Eight
A terminal-based sliding tile puzzle based on the game 2048. Tiles of matching values double and combine when slid, and players must achieve a 2048 tile before the grid fills up.

80. Vigenère Cipher
A multi-key polyalphabetic substitution cipher. It encrypts and decrypts messages by shifting characters based on repeating offsets calculated from a secret keyword.

81. Water Bucket Puzzle
A solitaire puzzle game where the player must measure out exactly four liters of water. It utilizes three buckets of different capacities (three, five, and eight liters) and counts the steps taken to reach the goal.
