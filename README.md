# Coach

## Usage

When the results of the meet are published:

1. Inform the ID of the meet on Swimming Canada's website
2. Upload a CSV file with the meet entries
3. Submit the form

The application will:

1. Store the meet entries in the database
2. Visit the meet results on Swimming Canada's website and store in the database
3. Query the database to compare the entry times with the results considering the right course of the event
4. Generate the report on an HTML page

The report contains:

1. List of swimmers who made best times ordered by the biggest to the smaller improvement
2. Each row shows:
    - Full name
    - Age
    - Course
    - Previous Best Time
    - New Best Time
    - Difference

## Meet Entries

Fields from the Entry File:

* SwimmerId -> swimmer.id
* Name -> swimmer.name
* Gender -> swimmer.gender
* DOB -> swimmer.birth_date
* Event -> event.distance + event.style
* Best Time Short -> swimmer_time.best_time + swimmer.course
* Best Time Date Short -> swimmer_time.date_taken
* Best Time Long -> swimmer_time.best_time + swimmer.course
* Best Time Date Long - swimmer_time.time_taken